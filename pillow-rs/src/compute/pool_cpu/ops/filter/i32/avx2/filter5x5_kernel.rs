//! Eight-lane I-mode 5x5 convolution used by the AVX2/FMA strategy.

use wide::f32x8;

#[inline]
fn native_filter_5x5_i32_vector(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [i32; 8] {
    if x_start + 8 <= width - 2 {
        let row = |dy: isize, kernel_start: usize| -> f32x8 {
            let source_y = (y as isize + dy) as usize;
            let first_pixel = source_y * width + x_start - 2;
            let first_start = first_pixel * 4;
            let second_start = (first_pixel + 4) * 4;
            let first_pixels: [i32; 8] =
                bytemuck::pod_read_unaligned(&raw[first_start..first_start + 8 * 4]);
            let second_pixels: [i32; 8] =
                bytemuck::pod_read_unaligned(&raw[second_start..second_start + 8 * 4]);
            let first = first_pixels.map(|pixel| i32::from_le(pixel) as f32);
            let second = second_pixels.map(|pixel| i32::from_le(pixel) as f32);
            let taps: [f32x8; 5] = std::array::from_fn(|tap| {
                f32x8::from(std::array::from_fn(|lane| {
                    let sample_index = tap + lane;
                    if sample_index < 8 {
                        first[sample_index]
                    } else {
                        second[sample_index - 4]
                    }
                }))
            });
            let sum = taps[1] * f32x8::splat(kernel[kernel_start + 1]);
            let sum = taps[0].mul_add(f32x8::splat(kernel[kernel_start]), sum);
            let sum = taps[2].mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
            let sum = taps[3].mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
            taps[4].mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
        };

        let mut total = f32x8::splat(rounding_bias);
        total += row(2, 0);
        total += row(1, 5);
        total += row(0, 10);
        total += row(-1, 15);
        total += row(-2, 20);
        let values = total.to_array();
        return std::array::from_fn(|lane| {
            if values[lane] >= 0.0 {
                values[lane] as i32
            } else {
                0
            }
        });
    }

    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [[0.0f32; 8]; 5];
        let source_y = (y as isize + dy) as usize;
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 3);
            for tap in 0..5 {
                let source_x = x + tap - 2;
                let base = (source_y * width + source_x) * 4;
                samples[tap][lane] =
                    i32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]])
                        as f32;
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
        if values[lane] >= 0.0 {
            values[lane] as i32
        } else {
            0
        }
    })
}

/// Applies the vector row strategy while preserving the copied I-mode border.
pub(crate) fn native_filter_5x5_i32_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let byte_count = width.checked_mul(height)?.checked_mul(4)?;
    if raw.len() != byte_count || out.len() != byte_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }
    let interior_width = width - 4;
    let interior_height = height - 4;
    let vector_blocks = interior_width.div_ceil(8).saturating_mul(interior_height);
    let row_stride = width * 4;
    for y in 2..height - 2 {
        let row_start = y * row_stride;
        let destination = &mut out[row_start..row_start + row_stride];
        let mut x = 2usize;
        while x < width - 2 {
            let active = (width - 2 - x).min(8);
            let values = native_filter_5x5_i32_vector(raw, width, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate().take(active) {
                let base = (x + lane) * 4;
                destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += active;
        }
    }
    Some((vector_blocks as u64, 0))
}
