//! Fused native-byte Sharpness kernels for declared CPU architecture strategies.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use wide::{f32x8, i16x8, u8x16, u16x8};

#[derive(Clone, Copy)]
struct NativeSharpnessPlan<'a, const CHANNELS: usize, const ACTIVE: usize> {
    source: &'a [u8],
    width: usize,
    height: usize,
    row_stride: usize,
    kernel: &'a [f32; 9],
    rounding_bias: f32,
    alpha: f32,
    grouped_rgb_bytes: bool,
    grouped_pixels: bool,
}

/// Apply the archived fused vector filter and blend to one exact byte layout.
/// Const parameters are supplied by the owning mode strategy so channel
/// layout decisions are fixed before the pixel loop is entered.
pub(crate) fn execute_image<
    P,
    const CHANNELS: usize,
    const ACTIVE: usize,
    const GROUP_RGB_BYTES: bool,
    const GROUP_PIXELS: bool,
>(
    source: &ImageBuffer<P, Vec<u8>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if CHANNELS == 0 || CHANNELS > 4 || ACTIVE == 0 || ACTIVE > CHANNELS {
        return Err(PilError::InternalError(
            "Sharpness strategy declared an invalid vector byte layout".into(),
        ));
    }
    if factor == 1.0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(source.clone());
    }

    let width = source.width() as usize;
    let height = source.height() as usize;
    let kernel = [
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        5.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
    ];
    let mut output = source.as_raw().to_vec();
    let (vector_blocks, scalar_tail) =
        filter_blend::<CHANNELS, ACTIVE, GROUP_RGB_BYTES, GROUP_PIXELS>(
            source.as_raw(),
            &mut output,
            width,
            height,
            &kernel,
            0.5,
            factor,
        )
        .ok_or_else(|| PilError::ValueError("enhance_sharpness: buffer error".into()))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::ValueError("enhance_sharpness: buffer error".into()))
}

#[inline]
fn filter_blend<
    const CHANNELS: usize,
    const ACTIVE: usize,
    const GROUP_RGB_BYTES: bool,
    const GROUP_PIXELS: bool,
>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
    factor: f64,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(CHANNELS)?;
    if source.len() != output.len()
        || source.len() != row_stride.checked_mul(height)?
        || width < 3
        || height < 3
    {
        return None;
    }
    let alpha = factor as f32;
    let interior_width = width - 2;
    let interior_height = height - 2;
    let grouped_rgb_bytes = GROUP_RGB_BYTES && alpha.is_finite();
    let grouped_pixels = GROUP_PIXELS && alpha.is_finite();
    let pixels_per_block = (8 / CHANNELS).max(1);
    let vector_blocks = if grouped_rgb_bytes {
        (interior_width * CHANNELS).div_ceil(8) * interior_height
    } else if grouped_pixels {
        interior_width
            .div_ceil(pixels_per_block)
            .saturating_mul(interior_height)
    } else {
        interior_width
            .div_ceil(8)
            .saturating_mul(interior_height)
            .saturating_mul(ACTIVE)
    };
    if vector_blocks == 0 {
        return None;
    }
    let scalar_tail = (width * 2 + height * 2 - 4).saturating_mul(ACTIVE);
    let plan = NativeSharpnessPlan::<CHANNELS, ACTIVE> {
        source,
        width,
        height,
        row_stride,
        kernel,
        rounding_bias,
        alpha,
        grouped_rgb_bytes,
        grouped_pixels,
    };
    for y in 0..height {
        let row_start = y * row_stride;
        blend_row(&plan, y, &mut output[row_start..row_start + row_stride]);
    }
    Some((vector_blocks as u64, scalar_tail as u64))
}

#[inline]
fn blend_row<const CHANNELS: usize, const ACTIVE: usize>(
    plan: &NativeSharpnessPlan<'_, CHANNELS, ACTIVE>,
    y: usize,
    row: &mut [u8],
) {
    if !(1..plan.height - 1).contains(&y) {
        let source_row = y * plan.row_stride;
        for x in 0..plan.width {
            for channel in 0..ACTIVE {
                let index = source_row + x * CHANNELS + channel;
                row[x * CHANNELS + channel] =
                    blend_value(plan.source[index], plan.source[index], plan.alpha);
            }
        }
    } else if plan.grouped_rgb_bytes {
        blend_rgb_row(plan, y, row);
    } else if plan.grouped_pixels {
        blend_grouped_pixels_row(plan, y, row);
    } else {
        blend_channels_row(plan, y, row);
    }
}

#[inline]
fn blend_rgb_row<const CHANNELS: usize, const ACTIVE: usize>(
    plan: &NativeSharpnessPlan<'_, CHANNELS, ACTIVE>,
    y: usize,
    row: &mut [u8],
) {
    // RGB bytes are contiguous across channels, so neighboring pixels can be
    // filtered as one byte stream without per-channel gathers.
    let interior_end = (plan.width - 1) * CHANNELS;
    let source_row = y * plan.row_stride;
    let mut byte_offset = CHANNELS;
    while byte_offset < interior_end {
        let active_bytes = (interior_end - byte_offset).min(8);
        let smooth = filter_rgb_byte_block(plan.source, plan.width, y, byte_offset, active_bytes);
        let original = load_byte_block(plan.source, source_row + byte_offset, active_bytes);
        let values = blend_lanes(original, smooth, plan.alpha).to_array();
        for (lane, value) in values.into_iter().enumerate().take(active_bytes) {
            row[byte_offset + lane] = blend_result(value);
        }
        byte_offset += active_bytes;
    }
}

#[inline]
fn blend_grouped_pixels_row<const CHANNELS: usize, const ACTIVE: usize>(
    plan: &NativeSharpnessPlan<'_, CHANNELS, ACTIVE>,
    y: usize,
    row: &mut [u8],
) {
    // RGBX treats the stored X byte as data. Grouping whole pixels packs the
    // strided four-channel gathers into one eight-lane block.
    let source_row = y * plan.row_stride;
    let pixels_per_block = (8 / CHANNELS).max(1);
    let mut x = 1usize;
    while x < plan.width - 1 {
        let active_pixels = (plan.width - 1 - x).min(pixels_per_block);
        let active_bytes = active_pixels * CHANNELS;
        let smooth =
            filter_pixel_block_integer(plan.source, plan.width, CHANNELS, y, x, active_bytes);
        let source_start = source_row + x * CHANNELS;
        let original = std::array::from_fn(|lane| {
            if lane < active_bytes {
                plan.source[source_start + lane]
            } else {
                0
            }
        });
        let values = blend_integer_block(original, smooth, plan.alpha);
        for pixel in 0..active_pixels {
            for channel in 0..CHANNELS {
                let lane = pixel * CHANNELS + channel;
                row[(x + pixel) * CHANNELS + channel] = values[lane];
            }
        }
        x += active_pixels;
    }
}

#[inline]
fn blend_channels_row<const CHANNELS: usize, const ACTIVE: usize>(
    plan: &NativeSharpnessPlan<'_, CHANNELS, ACTIVE>,
    y: usize,
    row: &mut [u8],
) {
    let source_row = y * plan.row_stride;
    for channel in 0..ACTIVE {
        let first = source_row + channel;
        row[channel] = blend_value(plan.source[first], plan.source[first], plan.alpha);
        let mut x = 1usize;
        while x < plan.width - 1 {
            let active = (plan.width - 1 - x).min(8);
            let smooth = if plan.alpha.is_finite() {
                if CHANNELS == 1 && active == 8 {
                    filter_luma_block_integer(plan.source, plan.width, y, x).unwrap_or_else(|| {
                        filter_channel_block_integer(
                            plan.source,
                            plan.width,
                            CHANNELS,
                            channel,
                            y,
                            x,
                            active,
                        )
                    })
                } else {
                    filter_channel_block_integer(
                        plan.source,
                        plan.width,
                        CHANNELS,
                        channel,
                        y,
                        x,
                        active,
                    )
                }
            } else {
                // A finite f64 can overflow when narrowed to f32. Preserve the
                // archived floating gather and its NaN border behavior there.
                f32x8::from(
                    filter_channel_vector(
                        plan.source,
                        plan.width,
                        CHANNELS,
                        channel,
                        y,
                        x,
                        plan.kernel,
                        plan.rounding_bias,
                    )
                    .map(f32::from),
                )
            };
            let original = std::array::from_fn(|lane| {
                let pixel_x = (x + lane).min(plan.width - 2);
                f32::from(plan.source[source_row + pixel_x * CHANNELS + channel])
            });
            let values = blend_lanes(f32x8::from(original), smooth, plan.alpha).to_array();
            for (lane, value) in values.into_iter().enumerate().take(active) {
                row[(x + lane) * CHANNELS + channel] = blend_result(value);
            }
            x += active;
        }
        let last = source_row + (plan.width - 1) * CHANNELS + channel;
        row[(plan.width - 1) * CHANNELS + channel] =
            blend_value(plan.source[last], plan.source[last], plan.alpha);
    }
}

#[inline]
fn filter_channel_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    // Keep the archived middle-first multiply-add order. The last lane repeats
    // the final interior sample and is discarded when the block is partial.
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut left = [0.0f32; 8];
        let mut middle = [0.0f32; 8];
        let mut right = [0.0f32; 8];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 2);
            let row = (y as isize + dy) as usize * width;
            let left_index = (row + x - 1) * channels;
            let middle_index = (row + x) * channels;
            let right_index = (row + x + 1) * channels;
            left[lane] = f32::from(raw[left_index + channel]);
            middle[lane] = f32::from(raw[middle_index + channel]);
            right[lane] = f32::from(raw[right_index + channel]);
        }
        let sum = f32x8::from(middle) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(left).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        f32x8::from(right).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };
    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    total.to_array().map(|value| {
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

#[inline]
fn filter_pixel_block_integer(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_bytes: usize,
) -> [u8; 8] {
    // The weighted byte sum is at most 3315; adding six before division by 13
    // exactly rounds the intermediate SMOOTH byte to nearest.
    let load = |start: usize| {
        u16x8::new(std::array::from_fn(|lane| {
            if lane < active_bytes {
                u16::from(raw[start + lane])
            } else {
                0
            }
        }))
    };
    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * channels;
        let left = source_row + (x_start - 1) * channels;
        let middle = left + channels;
        let right = middle + channels;
        (load(left), load(middle), load(right))
    };
    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    (weighted + u16x8::splat(6))
        .to_array()
        .map(|value| (value / 13) as u8)
}

#[inline]
fn filter_channel_block_integer(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
) -> f32x8 {
    // Each u16 lane holds the exact weighted byte sum before the same Pillow
    // intermediate-byte rounding boundary used by the scalar Default path.
    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * channels;
        let load = |offset: isize| {
            u16x8::new(std::array::from_fn(|lane| {
                if lane < active_pixels {
                    let x = (x_start + lane) as isize + offset;
                    u16::from(raw[source_row + x as usize * channels + channel])
                } else {
                    0
                }
            }))
        };
        (load(-1), load(0), load(1))
    };
    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    f32x8::from(std::array::from_fn(|lane| (rounded[lane] / 13) as f32))
}

#[inline]
fn filter_luma_block_integer(raw: &[u8], width: usize, y: usize, x_start: usize) -> Option<f32x8> {
    // A contiguous 16-byte window supplies eight L samples and their taps by
    // shuffle, avoiding nine gathers for the one-channel strategy.
    const LEFT: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0];
    const CENTER: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0];
    const RIGHT: [u8; 16] = [2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 0, 0, 0, 0, 0];
    if width < 3 || x_start < 1 || x_start + 15 > width {
        return None;
    }
    let load_row = |row: usize| -> Option<(u16x8, u16x8, u16x8)> {
        let start = row.checked_mul(width)?.checked_add(x_start - 1)?;
        let window: [u8; 16] = raw.get(start..start.checked_add(16)?)?.try_into().ok()?;
        let window = u8x16::new(window);
        Some((
            u16x8::from_u8x16_low(window.shuffle(u8x16::new(LEFT))),
            u16x8::from_u8x16_low(window.shuffle(u8x16::new(CENTER))),
            u16x8::from_u8x16_low(window.shuffle(u8x16::new(RIGHT))),
        ))
    };
    let (below_left, below_middle, below_right) = load_row(y + 1)?;
    let (center_left, center_middle, center_right) = load_row(y)?;
    let (above_left, above_middle, above_right) = load_row(y - 1)?;
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    Some(f32x8::from(std::array::from_fn(|lane| {
        (rounded[lane] / 13) as f32
    })))
}

#[inline]
fn filter_rgb_byte_block(
    raw: &[u8],
    width: usize,
    y: usize,
    start_byte: usize,
    active_bytes: usize,
) -> f32x8 {
    // The three-byte stride follows each RGB channel through its neighbors;
    // each byte lane retains the same rounded intermediate as other modes.
    let load = |start: usize| {
        u16x8::new(std::array::from_fn(|lane| {
            if lane < active_bytes {
                u16::from(raw[start + lane])
            } else {
                0
            }
        }))
    };
    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * 3;
        let middle = source_row + start_byte;
        (load(middle - 3), load(middle), load(middle + 3))
    };
    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    f32x8::from(std::array::from_fn(|lane| (rounded[lane] / 13) as f32))
}

#[inline]
fn load_byte_block(raw: &[u8], start: usize, active_bytes: usize) -> f32x8 {
    f32x8::from(std::array::from_fn(|lane| {
        if lane < active_bytes {
            f32::from(raw[start + lane])
        } else {
            0.0
        }
    }))
}

#[inline]
fn blend_value(original: u8, smooth: u8, alpha: f32) -> u8 {
    blend_result(alpha.mul_add(f32::from(original) - f32::from(smooth), f32::from(smooth)))
}

#[inline]
fn blend_lanes(original: f32x8, smooth: f32x8, alpha: f32) -> f32x8 {
    // Match Image.blend's single-rounding FMA while using vector lanes where
    // the compiled architecture exposes packed FMA.
    let delta = original - smooth;
    let alpha = f32x8::splat(alpha);
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        alpha.mul_add(delta, smooth)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let alpha = alpha.to_array();
        let delta = delta.to_array();
        let smooth = smooth.to_array();
        f32x8::new(std::array::from_fn(|lane| {
            alpha[lane].mul_add(delta[lane], smooth[lane])
        }))
    }
}

#[inline]
fn blend_integer_block(original: [u8; 8], smooth: [u8; 8], alpha: f32) -> [u8; 8] {
    // Common binary factors have exact integer forms on byte inputs; retain
    // their clamp and truncation behavior without float blend lanes.
    if alpha == 0.0 {
        return smooth;
    }
    let original = i16x8::new(original.map(i16::from));
    let smooth_lanes = i16x8::new(smooth.map(i16::from));
    let blended = if alpha == 0.5 {
        (original + smooth_lanes) >> 1u32
    } else if alpha == 1.5 {
        ((original * i16x8::splat(3) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(510)))
            >> 1u32
    } else if alpha == 2.0 {
        (original * i16x8::splat(2) - smooth_lanes)
            .max(i16x8::splat(0))
            .min(i16x8::splat(255))
    } else {
        return blend_lanes(
            f32x8::from(original.to_array().map(f32::from)),
            f32x8::from(smooth_lanes.to_array().map(f32::from)),
            alpha,
        )
        .to_array()
        .map(blend_result);
    };
    blended
        .to_array()
        .map(|value| u8::try_from(value).unwrap_or_default())
}

#[inline]
fn blend_result(value: f32) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}
