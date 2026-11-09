//! Shared vector kernels for native-byte 3x3 filtering.

use wide::{f32x8, i32x8, u8x16, u16x8, u16x16};

// TODO(pillow-rs#3ptb): freeze byte Filter3x3 output and architecture-fallback
// receipts across all exact modes using this shared native-byte kernel.

/// Apply the archived native-byte vector convolution without changing mode.
pub(crate) fn apply(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(channels)?;
    let byte_count = row_stride.checked_mul(height)?;
    if !(1..=4).contains(&channels) || raw.len() != byte_count || output.len() != byte_count {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }
    if channels == 3 {
        return apply_rgb_stream(raw, output, width, height, kernel, rounding_bias);
    }
    Some(apply_rows(
        raw,
        output,
        width,
        height,
        channels,
        kernel,
        rounding_bias,
    ))
}

/// Apply the archived packed LA binomial kernel to eight pixels at a time.
/// The rolling row sums keep the exact integer rounding and copied border.
pub(crate) fn apply_la_binomial(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let row_samples = width.checked_mul(2)?;
    let sample_count = row_samples.checked_mul(height)?;
    if raw.len() != sample_count || output.len() != sample_count {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }

    let mut horizontal_rows: [Vec<u16>; 3] = std::array::from_fn(|_| vec![0; row_samples]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_samples;
        let mut x = 1usize;
        while x < width - 1 {
            let active_bytes = (width - 1 - x).min(8) * 2;
            let load = |pixel_x: usize| {
                let start = row_start + pixel_x * 2;
                let mut bytes = [0u8; 16];
                bytes[..active_bytes].copy_from_slice(&raw[start..start + active_bytes]);
                u16x16::from(u8x16::new(bytes))
            };
            let horizontal = load(x - 1) + load(x) * u16x16::splat(2) + load(x + 1);
            let values = horizontal.to_array();
            let output_start = x * 2;
            destination[output_start..output_start + active_bytes]
                .copy_from_slice(&values[..active_bytes]);
            x += active_bytes / 2;
        }
    };

    for source_y in 0..3 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 1..height - 1 {
        let above = &horizontal_rows[(y - 1) % 3];
        let center = &horizontal_rows[y % 3];
        let below = &horizontal_rows[(y + 1) % 3];
        let output_row = y * row_samples;
        let mut x = 1usize;
        while x < width - 1 {
            let active_bytes = (width - 1 - x).min(8) * 2;
            let sample_start = x * 2;
            let load = |row: &[u16]| {
                u16x16::new(std::array::from_fn(|lane| {
                    if lane < active_bytes {
                        row[sample_start + lane]
                    } else {
                        0
                    }
                }))
            };
            let weighted = load(above) + load(center) * u16x16::splat(2) + load(below);
            let rounded = (weighted + u16x16::splat(8)) >> 4u32;
            let bytes = crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(rounded)
                .to_array();
            output[output_row + sample_start..output_row + sample_start + active_bytes]
                .copy_from_slice(&bytes[..active_bytes]);
            x += active_bytes / 2;
        }

        let next_source_y = y + 2;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 3]);
        }
    }
    Some((((width - 2).div_ceil(8) * (height - 2)) as u64, 0))
}

fn apply_rows(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> (u64, u64) {
    let interior_width = width - 2;
    let interior_height = height - 2;
    let grouped_pixels = matches!(channels, 2 | 4);
    let block_pixels = (8 / channels).max(1);
    let blocks_per_row = if grouped_pixels {
        interior_width.div_ceil(block_pixels)
    } else {
        interior_width.div_ceil(8) * channels
    };
    let row_stride = width * channels;

    for y in 1..height - 1 {
        let row_start = y * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        if grouped_pixels {
            apply_grouped_row(
                raw,
                row,
                width,
                channels,
                y,
                block_pixels,
                kernel,
                rounding_bias,
            );
        } else {
            apply_channel_rows(raw, row, width, channels, y, kernel, rounding_bias);
        }
    }

    ((blocks_per_row * interior_height) as u64, 0)
}

fn apply_grouped_row(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    channels: usize,
    y: usize,
    block_pixels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    let mut x = 1;
    while x < width - 1 {
        let active_pixels = (width - 1 - x).min(block_pixels);
        let values = pixel_block(
            raw,
            width,
            channels,
            y,
            x,
            active_pixels,
            kernel,
            rounding_bias,
        );
        for pixel in 0..active_pixels {
            for channel in 0..channels {
                row[(x + pixel) * channels + channel] = values[pixel * channels + channel];
            }
        }
        x += active_pixels;
    }
}

fn apply_channel_rows(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    channels: usize,
    y: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    for channel in 0..channels {
        let mut x = 1;
        while x < width - 1 {
            let active = (width - 1 - x).min(8);
            let values = channel_block(raw, width, channels, channel, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate().take(active) {
                row[(x + lane) * channels + channel] = value;
            }
            x += active;
        }
    }
}

fn channel_block(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut left = [0.0; 8];
        let mut middle = [0.0; 8];
        let mut right = [0.0; 8];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 2);
            let row_start = (y as isize + dy) as usize * width;
            let base = (row_start + x) * channels;
            left[lane] = raw[base - channels + channel] as f32;
            middle[lane] = raw[base + channel] as f32;
            right[lane] = raw[base + channels + channel] as f32;
        }
        let sum = f32x8::from(middle) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(left).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        f32x8::from(right).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    total.to_array().map(clip_byte)
}

fn pixel_block(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let row_start = (y as isize + dy) as usize * width;
        let active_bytes = active_pixels * channels;
        let source_start = (row_start + x_start - 1) * channels;
        let left = load_byte_block(raw, source_start, active_bytes);
        let middle = load_byte_block(raw, source_start + channels, active_bytes);
        let right = load_byte_block(raw, source_start + 2 * channels, active_bytes);
        let sum = middle * f32x8::splat(kernel[kernel_start + 1]);
        let sum = left.mul_add(f32x8::splat(kernel[kernel_start]), sum);
        right.mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    total.to_array().map(clip_byte)
}

fn load_byte_block(raw: &[u8], start: usize, active_bytes: usize) -> f32x8 {
    let mut values = [0.0; 8];
    for lane in 0..active_bytes {
        values[lane] = raw[start + lane] as f32;
    }
    f32x8::from(values)
}

fn apply_rgb_stream(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(3)?;
    let byte_count = row_stride.checked_mul(height)?;
    if width < 3 || height < 3 || raw.len() != byte_count || output.len() != byte_count {
        return None;
    }
    let interior_bytes = row_stride - 6;
    let blocks_per_row = interior_bytes.div_ceil(8);
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        let mut output_byte = 3;
        let interior_end = row_stride - 3;
        while output_byte < interior_end {
            let active = (interior_end - output_byte).min(8);
            let values = rgb_byte_block(
                raw,
                row_stride,
                y,
                output_byte,
                active,
                kernel,
                rounding_bias,
            );
            row[output_byte..output_byte + active].copy_from_slice(&values[..active]);
            output_byte += active;
        }
    }
    Some(((blocks_per_row * (height - 2)) as u64, 0))
}

fn rgb_byte_block(
    raw: &[u8],
    row_stride: usize,
    y: usize,
    output_byte_start: usize,
    active_bytes: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let start = (y as isize + dy) as usize * row_stride + output_byte_start;
        let left = load_rgb_block(raw, start - 3, active_bytes);
        let middle = load_rgb_block(raw, start, active_bytes);
        let right = load_rgb_block(raw, start + 3, active_bytes);
        let sum = middle * f32x8::splat(kernel[kernel_start + 1]);
        let sum = left.mul_add(f32x8::splat(kernel[kernel_start]), sum);
        right.mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    total.to_array().map(clip_byte)
}

fn load_rgb_block(raw: &[u8], start: usize, active_bytes: usize) -> f32x8 {
    let mut padded = [0u8; 16];
    padded[..active_bytes].copy_from_slice(&raw[start..start + active_bytes]);
    let widened = u16x16::from(u8x16::new(padded));
    let [low, _high]: [u16x8; 2] = bytemuck::cast(widened);
    f32x8::from_i32x8(i32x8::from_u16x8(low))
}

fn clip_byte(value: f32) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}
