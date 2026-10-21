//! Native byte-layout Filter5x5 kernels shared by admitted modes.

#[cfg(feature = "parallel")]
use crate::compute::pool_simd::ops::adapters::simd_rows_mut_serial;
use crate::compute::pool_simd::ops::native_filter::native_filter_load_byte_block;
use wide::f32x8;

/// Evaluate eight or fewer interleaved output samples for the 5x5 kernel.
/// See [`native_filter_3x3_pixel_block_vector`] for why adjacent pixels are
/// grouped into one contiguous vector block.
#[inline]
fn native_filter_5x5_pixel_block_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [u8; 8] {
    debug_assert!((1..=4).contains(&channels));
    debug_assert!(active_pixels <= 8 / channels.max(1));
    debug_assert!(x_start >= 2 && x_start + active_pixels <= width - 2);

    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [f32x8::ZERO; 5];
        let source_row = (y as isize + dy) as usize * width;
        let active_bytes = active_pixels * channels;
        let source_start = (source_row + x_start - 2) * channels;
        for tap in 0..5 {
            samples[tap] =
                native_filter_load_byte_block(raw, source_start + tap * channels, active_bytes);
        }
        let sum = samples[1] * f32x8::splat(kernel[kernel_start + 1]);
        let sum = samples[0].mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = samples[2].mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = samples[3].mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        samples[4].mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(2, 0);
    total += row(1, 5);
    total += row(0, 10);
    total += row(-1, 15);
    total += row(-2, 20);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Evaluate eight output samples of Pillow's 5x5 byte convolution for one
/// channel.  This remains the lower-overhead layout for one-channel images
/// and RGB, where grouping complete interleaved pixels would add padding
/// lanes or increase the number of vector blocks.
#[inline]
fn native_filter_5x5_vector(
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
            let row = (y as isize + dy) as usize * width;
            for tap in 0..5 {
                samples[tap][lane] = raw[(row + x + tap - 2) * channels + channel] as f32;
            }
        }
        let sum = f32x8::from(samples[1]) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(samples[0]).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = f32x8::from(samples[2]).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = f32x8::from(samples[3]).mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        f32x8::from(samples[4]).mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(2, 0);
    total += row(1, 5);
    total += row(0, 10);
    total += row(-1, 15);
    total += row(-2, 20);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Apply the exact native-byte 5x5 convolution with eight-wide vector lanes.
pub(crate) fn native_filter_5x5_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    if !(1..=4).contains(&channels) || width < 5 || height < 5 {
        return;
    }
    let interior_width = width - 4;
    let interior_height = height - 4;
    let grouped_pixels = matches!(channels, 2 | 4);
    let pixel_block = (8 / channels).max(1);
    let vector_blocks = if grouped_pixels {
        interior_width
            .div_ceil(pixel_block)
            .saturating_mul(interior_height)
    } else {
        interior_width
            .div_ceil(8)
            .saturating_mul(channels)
            .saturating_mul(interior_height)
    };
    let scalar_tail = 0;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        if grouped_pixels {
            let mut x = 2usize;
            while x < width - 2 {
                let active_pixels = (width - 2 - x).min(pixel_block);
                let values = native_filter_5x5_pixel_block_vector(
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
        } else {
            for channel in 0..channels {
                let mut x = 2usize;
                while x < width - 2 {
                    let active = (width - 2 - x).min(8);
                    let values = native_filter_5x5_vector(
                        raw,
                        width,
                        channels,
                        channel,
                        y,
                        x,
                        kernel,
                        rounding_bias,
                    );
                    for (lane, value) in values.into_iter().enumerate().take(active) {
                        row[(x + lane) * channels + channel] = value;
                    }
                    x += active;
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    simd_rows_mut_serial!(out, row_stride, height, |_row_start, _row_end, y, row| {
        let y = y as usize;
        if (2..height - 2).contains(&y) {
            apply_row(y, row);
        }
    });
    #[cfg(not(feature = "parallel"))]
    for y in 2..height - 2 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
}
