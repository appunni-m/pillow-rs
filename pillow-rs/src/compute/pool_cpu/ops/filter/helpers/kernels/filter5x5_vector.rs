//! Shared vector kernels for native-byte 5x5 filtering.

use wide::f32x8;

/// Apply the archived interleaved-byte 5x5 convolution without changing mode.
pub(crate) fn apply(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let row_stride = width.checked_mul(channels)?;
    let byte_count = row_stride.checked_mul(height)?;
    if !(1..=4).contains(&channels) || raw.len() != byte_count || output.len() != byte_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }

    let interior_width = width - 4;
    let interior_height = height - 4;
    let grouped_pixels = matches!(channels, 2 | 4);
    let pixel_block = (8 / channels).max(1);
    let blocks_per_row = if grouped_pixels {
        interior_width.div_ceil(pixel_block)
    } else {
        interior_width.div_ceil(8) * channels
    };
    let row_stride = width * channels;
    for y in 2..height - 2 {
        let row_start = y * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        if grouped_pixels {
            apply_pixel_row(
                raw,
                row,
                width,
                channels,
                y,
                pixel_block,
                kernel,
                rounding_bias,
            );
        } else {
            apply_channel_row(raw, row, width, channels, y, kernel, rounding_bias);
        }
    }
    Some(((blocks_per_row * interior_height) as u64, 0))
}

fn apply_pixel_row(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    channels: usize,
    y: usize,
    pixel_block: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    let mut x = 2usize;
    while x < width - 2 {
        let active_pixels = (width - 2 - x).min(pixel_block);
        let values = pixel_block_values(
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

fn apply_channel_row(
    raw: &[u8],
    row: &mut [u8],
    width: usize,
    channels: usize,
    y: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    for channel in 0..channels {
        let mut x = 2usize;
        while x < width - 2 {
            let active = (width - 2 - x).min(8);
            let values = channel_block(raw, width, channels, channel, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate().take(active) {
                row[(x + lane) * channels + channel] = value;
            }
            x += active;
        }
    }
}

fn pixel_block_values(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let active_bytes = active_pixels * channels;
        let source_row = (y as isize + dy) as usize * width;
        let source_start = (source_row + x_start - 2) * channels;
        let mut samples = [f32x8::ZERO; 5];
        for tap in 0..5 {
            samples[tap] = load_byte_block(raw, source_start + tap * channels, active_bytes);
        }
        let sum = samples[1] * f32x8::splat(kernel[kernel_start + 1]);
        let sum = samples[0].mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = samples[2].mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = samples[3].mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        samples[4].mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };
    clip_lanes(sum_rows(row, rounding_bias))
}

fn channel_block(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [[0.0f32; 8]; 5];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 3);
            let source_row = (y as isize + dy) as usize * width;
            for tap in 0..5 {
                samples[tap][lane] = raw[(source_row + x + tap - 2) * channels + channel] as f32;
            }
        }
        let sum = f32x8::from(samples[1]) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(samples[0]).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = f32x8::from(samples[2]).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = f32x8::from(samples[3]).mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        f32x8::from(samples[4]).mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };
    clip_lanes(sum_rows(row, rounding_bias))
}

fn sum_rows(row: impl Fn(isize, usize) -> f32x8, rounding_bias: f32) -> [f32; 8] {
    let mut total = f32x8::splat(rounding_bias);
    total += row(2, 0);
    total += row(1, 5);
    total += row(0, 10);
    total += row(-1, 15);
    total += row(-2, 20);
    total.to_array()
}

fn load_byte_block(raw: &[u8], start: usize, active_bytes: usize) -> f32x8 {
    let mut values = [0.0f32; 8];
    for lane in 0..active_bytes {
        values[lane] = raw[start + lane] as f32;
    }
    f32x8::from(values)
}

fn clip_lanes(values: [f32; 8]) -> [u8; 8] {
    values.map(|value| {
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}
