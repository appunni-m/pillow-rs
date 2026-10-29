use super::{NativeReduceGeometry, SIMD_REDUCE_LANES, u32x8};
pub(super) fn native_reduce_pixel_sums(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    channels: usize,
    premultiplied_alpha: bool,
    x: usize,
    y: usize,
) -> Option<([u32; 4], u32, u32, u32)> {
    let (source_x, source_y, block_width, block_height, multiplier, amend) =
        geometry.block_parameters(x, y)?;
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        for dx in 0..block_width {
            let source_index = ((source_y + dy) * geometry.width + source_x + dx) * channels;
            for channel in 0..channels {
                let mut sample = u32::from(source[source_index + channel]);
                if premultiplied_alpha && channel + 1 < channels {
                    let alpha = u32::from(source[source_index + channels - 1]);
                    sample = (sample * alpha + 127) / 255;
                }
                sums[channel] = sums[channel].checked_add(u64::from(sample))?;
            }
        }
    }
    Some((
        [
            u32::try_from(sums[0]).ok()?,
            u32::try_from(sums[1]).ok()?,
            u32::try_from(sums[2]).ok()?,
            u32::try_from(sums[3]).ok()?,
        ],
        (block_width * block_height) as u32,
        multiplier,
        amend,
    ))
}

#[inline]
pub(super) fn native_reduce_average(sum: u32, multiplier: u32, amend: u32) -> u8 {
    (((u64::from(sum) + u64::from(amend)) * u64::from(multiplier)) >> 24).min(u64::from(u8::MAX))
        as u8
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_reduce_scalar_pixel(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    channels: usize,
    premultiplied_alpha: bool,
    x: usize,
    y: usize,
) -> Option<[u8; 4]> {
    let (sums, _count, multiplier, amend) =
        native_reduce_pixel_sums(source, geometry, channels, premultiplied_alpha, x, y)?;
    let mut output = [0u8; 4];
    let alpha = if premultiplied_alpha {
        native_reduce_average(sums[channels - 1], multiplier, amend)
    } else {
        u8::MAX
    };
    for channel in 0..channels {
        let value = native_reduce_average(sums[channel], multiplier, amend);
        output[channel] = if premultiplied_alpha && channel + 1 < channels {
            if alpha == 0 {
                value
            } else {
                (u16::from(value) * 255 / u16::from(alpha)) as u8
            }
        } else {
            value
        };
    }
    Some(output)
}

#[inline]
fn native_reduce_full_pixel_sums(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    channels: usize,
    premultiplied_alpha: bool,
    source_x: usize,
    source_y: usize,
    block_width: usize,
    block_height: usize,
) -> [u32; 4] {
    let mut sums = [0u32; 4];
    for dy in 0..block_height {
        for dx in 0..block_width {
            let source_index = ((source_y + dy) * geometry.width + source_x + dx) * channels;
            let alpha = if premultiplied_alpha {
                u32::from(source[source_index + channels - 1])
            } else {
                0
            };
            for channel in 0..channels {
                let sample = if premultiplied_alpha && channel + 1 < channels {
                    ((u32::from(source[source_index + channel]) * alpha + 127) / 255) as u8
                } else {
                    source[source_index + channel]
                };
                sums[channel] += u32::from(sample);
            }
        }
    }
    sums
}

// Eight output pixels from complete 2×2 blocks. Three-byte pixels prevent a
// single contiguous load from lining up with these channel sums, so gather the
// fixed offsets once and keep the averaging arithmetic in eight SIMD lanes.
#[inline]
pub(super) fn native_reduce_3byte_2x2_vector_block(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    output_x: usize,
    output_y: usize,
) -> Option<[u8; SIMD_REDUCE_LANES * 4]> {
    if geometry.x_factor != 2
        || geometry.y_factor != 2
        || output_x > geometry.main_width.saturating_sub(SIMD_REDUCE_LANES)
        || output_y >= geometry.main_height
    {
        return None;
    }
    let source_x = output_x.checked_mul(6)?;
    let source_top = output_y
        .checked_mul(2)?
        .checked_mul(geometry.width)?
        .checked_mul(3)?
        .checked_add(source_x)?;
    let source_bottom = source_top + geometry.width * 3;
    let channel_sums = |channel: usize| {
        std::array::from_fn(|lane| {
            let relative_index = lane * 6 + channel;
            let source_index = source_top + relative_index;
            u32::from(source[source_index])
                + u32::from(source[source_index + 3])
                + u32::from(source[source_bottom + relative_index])
                + u32::from(source[source_bottom + relative_index + 3])
        })
    };
    let averaged: [[u8; SIMD_REDUCE_LANES]; 3] = std::array::from_fn(|channel| {
        ((u32x8::new(channel_sums(channel)) + u32x8::splat(2)) >> 2u32)
            .to_array()
            .map(|value| value.min(255) as u8)
    });
    let mut output = [0u8; SIMD_REDUCE_LANES * 4];
    for lane in 0..SIMD_REDUCE_LANES {
        let destination = lane * 3;
        output[destination] = averaged[0][lane];
        output[destination + 1] = averaged[1][lane];
        output[destination + 2] = averaged[2][lane];
    }
    Some(output)
}

// Eight adjacent RGB destination pixels each consume a 3×5 source block.
// This keeps the RGB source walk explicit and factor-bounded; only the final
// rounded 24-bit reciprocal average is expressed as wide SIMD lanes. Partial
// blocks, other factors, thumbnail reduction, and other three-byte modes stay
// on the general path.
#[inline]
fn native_reduce_rgb_3x5_pixel_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
) -> [u32; 3] {
    let mut sums = [0u32; 3];
    let mut source_row = source_y * width * 3 + source_x * 3;
    for _ in 0..5 {
        let mut source_index = source_row;
        for _ in 0..3 {
            sums[0] += u32::from(source[source_index]);
            sums[1] += u32::from(source[source_index + 1]);
            sums[2] += u32::from(source[source_index + 2]);
            source_index += 3;
        }
        source_row += width * 3;
    }
    sums
}

#[inline]
pub(super) fn native_reduce_rgb_3x5_vector_block(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    output_x: usize,
    output_y: usize,
) -> Option<[u8; SIMD_REDUCE_LANES * 4]> {
    if geometry.x_factor != 3
        || geometry.y_factor != 5
        || output_x > geometry.main_width.saturating_sub(SIMD_REDUCE_LANES)
        || output_y >= geometry.main_height
    {
        return None;
    }
    let source_x = output_x.checked_mul(3)?;
    let source_y = output_y.checked_mul(5)?;
    let average = geometry.full?;
    let mut red_sums = [0u32; SIMD_REDUCE_LANES];
    let mut green_sums = [0u32; SIMD_REDUCE_LANES];
    let mut blue_sums = [0u32; SIMD_REDUCE_LANES];
    for lane in 0..SIMD_REDUCE_LANES {
        let sums =
            native_reduce_rgb_3x5_pixel_sums(source, geometry.width, source_x + lane * 3, source_y);
        red_sums[lane] = sums[0];
        green_sums[lane] = sums[1];
        blue_sums[lane] = sums[2];
    }

    let multiplier = u32x8::splat(average.multiplier);
    let amend = u32x8::splat(average.amend);
    let red: [u8; SIMD_REDUCE_LANES] = (((u32x8::new(red_sums) + amend) * multiplier) >> 24u32)
        .to_array()
        .map(|value| value.min(255) as u8);
    let green: [u8; SIMD_REDUCE_LANES] = (((u32x8::new(green_sums) + amend) * multiplier) >> 24u32)
        .to_array()
        .map(|value| value.min(255) as u8);
    let blue: [u8; SIMD_REDUCE_LANES] = (((u32x8::new(blue_sums) + amend) * multiplier) >> 24u32)
        .to_array()
        .map(|value| value.min(255) as u8);

    let mut output = [0u8; SIMD_REDUCE_LANES * 4];
    for lane in 0..SIMD_REDUCE_LANES {
        let destination = lane * 3;
        output[destination] = red[lane];
        output[destination + 1] = green[lane];
        output[destination + 2] = blue[lane];
    }
    Some(output)
}

#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_reduce_vector_block(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    output_x: usize,
    output_y: usize,
    valid_pixels: usize,
    channels: usize,
    premultiplied_alpha: bool,
    rgb_3x5_fast_path: bool,
) -> Option<[u8; SIMD_REDUCE_LANES * 4]> {
    if channels == 3
        && !premultiplied_alpha
        && valid_pixels == SIMD_REDUCE_LANES
        && geometry.x_factor == 2
        && geometry.y_factor == 2
        && output_x <= geometry.main_width.saturating_sub(SIMD_REDUCE_LANES)
        && output_y < geometry.main_height
    {
        return native_reduce_3byte_2x2_vector_block(source, geometry, output_x, output_y);
    }

    if rgb_3x5_fast_path
        && channels == 3
        && !premultiplied_alpha
        && valid_pixels == SIMD_REDUCE_LANES
        && geometry.x_factor == 3
        && geometry.y_factor == 5
        && output_x <= geometry.main_width.saturating_sub(SIMD_REDUCE_LANES)
        && output_y < geometry.main_height
    {
        return native_reduce_rgb_3x5_vector_block(source, geometry, output_x, output_y);
    }

    let mut sums = [[0u32; SIMD_REDUCE_LANES]; 4];
    let mut multipliers = [0u32; SIMD_REDUCE_LANES];
    let mut amends = [0u32; SIMD_REDUCE_LANES];
    let full_width_batch = valid_pixels != 0
        && output_x < geometry.main_width
        && valid_pixels <= geometry.main_width - output_x;
    let mut used_full_width_batch = false;
    if full_width_batch {
        let (source_x, source_y, block_width, block_height, multiplier, amend) =
            geometry.block_parameters(output_x, output_y)?;
        if block_width == geometry.x_factor {
            for lane in 0..valid_pixels {
                let pixel_sums = native_reduce_full_pixel_sums(
                    source,
                    geometry,
                    channels,
                    premultiplied_alpha,
                    source_x + lane * geometry.x_factor,
                    source_y,
                    block_width,
                    block_height,
                );
                for channel in 0..channels {
                    sums[channel][lane] = pixel_sums[channel];
                }
                multipliers[lane] = multiplier;
                amends[lane] = amend;
            }
            used_full_width_batch = true;
        }
    }
    if !used_full_width_batch {
        for lane in 0..valid_pixels {
            let (pixel_sums, _count, multiplier, amend) = native_reduce_pixel_sums(
                source,
                geometry,
                channels,
                premultiplied_alpha,
                output_x + lane,
                output_y,
            )?;
            for channel in 0..channels {
                sums[channel][lane] = pixel_sums[channel];
            }
            multipliers[lane] = multiplier;
            amends[lane] = amend;
        }
    }
    let multipliers = u32x8::new(multipliers);
    let amends = u32x8::new(amends);
    let mut averaged = [[0u8; SIMD_REDUCE_LANES]; 4];
    for channel in 0..channels {
        let values = ((u32x8::new(sums[channel]) + amends) * multipliers) >> 24u32;
        averaged[channel] = values.to_array().map(|value| value.min(255) as u8);
    }
    let mut output = [0u8; SIMD_REDUCE_LANES * 4];
    let has_alpha = premultiplied_alpha;
    for lane in 0..SIMD_REDUCE_LANES {
        let alpha = if has_alpha {
            averaged[channels - 1][lane]
        } else {
            u8::MAX
        };
        for channel in 0..channels {
            let value = averaged[channel][lane];
            output[lane * channels + channel] = if has_alpha && channel + 1 < channels && alpha != 0
            {
                (u16::from(value) * 255 / u16::from(alpha)) as u8
            } else {
                value
            };
        }
    }
    Some(output)
}
