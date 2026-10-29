//! Portable wide-lane implementation of the archived native-byte reducer.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use super::super::general::ByteReduceContext;
use wide::u32x8;

const LANES: usize = 8;
const LANES_U32: u32 = 8;
const RGB_BYTES: usize = 3;

#[inline]
fn clamp_u8(value: u32) -> u8 {
    u8::try_from(value.min(u32::from(u8::MAX))).unwrap_or(u8::MAX)
}

#[inline]
fn average_u32x8(
    sums: [u32; LANES],
    multipliers: [u32; LANES],
    amends: [u32; LANES],
) -> [u32; LANES] {
    (((u32x8::new(sums) + u32x8::new(amends)) * u32x8::new(multipliers)) >> 24u32)
        .to_array()
}

#[inline]
fn average_2x2_u32x8(sums: [u32; LANES]) -> [u32; LANES] {
    ((u32x8::new(sums) + u32x8::splat(2)) >> 2u32).to_array()
}

fn rgb_3x5_pixel_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
) -> [u32; RGB_BYTES] {
    let mut sums = [0u32; RGB_BYTES];
    let mut source_row = source_y * width * RGB_BYTES + source_x * RGB_BYTES;
    for _ in 0..5 {
        let mut source_index = source_row;
        for _ in 0..3 {
            sums[0] += u32::from(source[source_index]);
            sums[1] += u32::from(source[source_index + 1]);
            sums[2] += u32::from(source[source_index + 2]);
            source_index += RGB_BYTES;
        }
        source_row += width * RGB_BYTES;
    }
    sums
}

fn reduce_rgb_2x2_block(
    source: &[u8],
    width: usize,
    output_x: usize,
    output_y: usize,
    output: &mut [u8],
) {
    let source_x = output_x * 6;
    let source_top = output_y * 2 * width * RGB_BYTES + source_x;
    let source_bottom = source_top + width * RGB_BYTES;
    let channel_sums = |channel| {
        std::array::from_fn(|lane| {
            let relative_index = lane * 6 + channel;
            let source_index = source_top + relative_index;
            u32::from(source[source_index])
                + u32::from(source[source_index + 3])
                + u32::from(source[source_bottom + relative_index])
                + u32::from(source[source_bottom + relative_index + 3])
        })
    };
    let averages: [[u8; LANES]; RGB_BYTES] = std::array::from_fn(|channel| {
        average_2x2_u32x8(channel_sums(channel)).map(clamp_u8)
    });
    for lane in 0..LANES {
        let destination = lane * RGB_BYTES;
        output[destination] = averages[0][lane];
        output[destination + 1] = averages[1][lane];
        output[destination + 2] = averages[2][lane];
    }
}

fn reduce_rgb_3x5_block(
    source: &[u8],
    width: usize,
    output_x: usize,
    output_y: usize,
    rounding: (u32, u32),
    output: &mut [u8],
) {
    let (multiplier, amend) = rounding;
    let source_x = output_x * 3;
    let source_y = output_y * 5;
    let mut channel_sums = [[0u32; LANES]; RGB_BYTES];
    for lane in 0..LANES {
        let sums = rgb_3x5_pixel_sums(source, width, source_x + lane * 3, source_y);
        for channel in 0..RGB_BYTES {
            channel_sums[channel][lane] = sums[channel];
        }
    }
    let multipliers = [multiplier; LANES];
    let amends = [amend; LANES];
    let averages: [[u8; LANES]; RGB_BYTES] = std::array::from_fn(|channel| {
        average_u32x8(channel_sums[channel], multipliers, amends).map(clamp_u8)
    });
    for lane in 0..LANES {
        let destination = lane * RGB_BYTES;
        output[destination] = averages[0][lane];
        output[destination + 1] = averages[1][lane];
        output[destination + 2] = averages[2][lane];
    }
}

fn write_special_rgb_vector<const CHANNELS: usize, const PREMULTIPLIED: bool, const RGB_3X5: bool>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    factors: (u32, u32),
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> bool {
    let (main_width, main_height) = context.main_dimensions();
    let full_rgb_vector = CHANNELS == 3
        && !PREMULTIPLIED
        && valid_pixels == LANES
        && main_width >= LANES_U32
        && output_x <= main_width - LANES_U32
        && output_y < main_height;
    if full_rgb_vector && factors == (2, 2) {
        reduce_rgb_2x2_block(
            context.source(),
            context.source_width() as usize,
            output_x as usize,
            output_y as usize,
            output,
        );
        return true;
    }
    if full_rgb_vector && RGB_3X5 && factors == (3, 5) {
        let Some(rounding) = context.full_width_rounding(output_y) else {
            return false;
        };
        reduce_rgb_3x5_block(
            context.source(),
            context.source_width() as usize,
            output_x as usize,
            output_y as usize,
            rounding,
            output,
        );
        return true;
    }
    false
}

fn write_generic_vector<const CHANNELS: usize, const PREMULTIPLIED: bool>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (main_width, _) = context.main_dimensions();
    let full_width_batch = valid_pixels != 0
        && output_x < main_width
        && valid_pixels as u32 <= main_width - output_x;
    let full_width_rounding = if full_width_batch {
        Some(context.full_width_rounding(output_y).ok_or_else(|| {
            PilError::InternalError("Reduce wide full-row rounding facts were unavailable".into())
        })?)
    } else {
        None
    };
    let mut sums = [[0u32; LANES]; 4];
    let mut multipliers = [0u32; LANES];
    let mut amends = [0u32; LANES];
    for lane in 0..valid_pixels {
        let lane_x = output_x + lane as u32;
        let (pixel_sums, multiplier, amend) = if let Some((multiplier, amend)) = full_width_rounding {
            let sums = context.full_width_sums(lane_x, output_y).ok_or_else(|| {
                PilError::InternalError("Reduce wide full-row bounds were invalid".into())
            })?;
            (sums, multiplier, amend)
        } else {
            context.pixel_sums(lane_x, output_y).ok_or_else(|| {
                PilError::InternalError("Reduce wide edge block facts were unavailable".into())
            })?
        };
        for channel in 0..CHANNELS {
            sums[channel][lane] = pixel_sums[channel];
        }
        multipliers[lane] = multiplier;
        amends[lane] = amend;
    }
    let mut averages = [[0u8; LANES]; 4];
    for channel in 0..CHANNELS {
        averages[channel] = average_u32x8(sums[channel], multipliers, amends).map(clamp_u8);
    }
    write_averaged_pixels::<CHANNELS, PREMULTIPLIED>(&averages, valid_pixels, output);
    Ok(())
}

fn write_averaged_pixels<const CHANNELS: usize, const PREMULTIPLIED: bool>(
    averages: &[[u8; LANES]; 4],
    valid_pixels: usize,
    output: &mut [u8],
) {
    for lane in 0..valid_pixels {
        let alpha = if PREMULTIPLIED {
            averages[CHANNELS - 1][lane]
        } else {
            u8::MAX
        };
        for channel in 0..CHANNELS {
            let value = averages[channel][lane];
            let output_value = if PREMULTIPLIED && channel + 1 < CHANNELS && alpha != 0 {
                let value = u16::from(value) * 255 / u16::from(alpha);
                u8::try_from(value).unwrap_or(u8::MAX)
            } else {
                value
            };
            output[lane * CHANNELS + channel] = output_value;
        }
    }
}

fn reduce_vector_block<const CHANNELS: usize, const PREMULTIPLIED: bool, const RGB_3X5: bool>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    factors: (u32, u32),
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    if write_special_rgb_vector::<CHANNELS, PREMULTIPLIED, RGB_3X5>(
        context,
        factors,
        output_x,
        output_y,
        valid_pixels,
        output,
    ) {
        return Ok(());
    }
    write_generic_vector(context, output_x, output_y, valid_pixels, output)
}

/// Runs the archived portable wide-lane reducer for one exact mode strategy.
pub(crate) fn reduce<const CHANNELS: usize, const PREMULTIPLIED: bool, const RGB_3X5: bool>(
    source: &[u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let area = x_factor.checked_mul(y_factor).ok_or_else(|| {
        PilError::InternalError("Reduce wide factor area overflowed after selection".into())
    })?;
    if area > u32::MAX / 256 {
        return Err(PilError::InternalError(
            "Reduce wide factor area exceeded the lane-sum bound".into(),
        ));
    }
    let context = ByteReduceContext::<CHANNELS, PREMULTIPLIED>::new(
        source, width, height, x_factor, y_factor,
    );
    let (output_width, output_height) = context.output_dimensions();
    let mut output = CheckedDims::new(output_width, output_height, CHANNELS as u8)?.alloc_buffer();
    if output_width == 0 || output_height == 0 {
        return Ok(output);
    }
    let output_width_usize = output_width as usize;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "CheckedDims validated the complete tightly packed output buffer"
    )]
    let row_stride = output_width_usize * CHANNELS;
    let vector_limit = output_width / LANES_U32 * LANES_U32;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "CheckedDims validated each row offset and the final row end"
    )]
    for y in 0..output_height {
        let row_start = y as usize * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        for local in (0..vector_limit).step_by(LANES) {
            let valid_pixels = (output_width - local).min(LANES_U32) as usize;
            let start = local as usize * CHANNELS;
            let end = start + valid_pixels * CHANNELS;
            reduce_vector_block::<CHANNELS, PREMULTIPLIED, RGB_3X5>(
                &context,
                (x_factor, y_factor),
                local,
                y,
                valid_pixels,
                &mut row[start..end],
            )?;
        }
        context.write_row_range(y, row, vector_limit, output_width);
    }
    Ok(output)
}
