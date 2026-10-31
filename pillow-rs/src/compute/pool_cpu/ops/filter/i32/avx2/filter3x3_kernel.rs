//! Four-lane I32 convolution kernel retained from the SIMD implementation.

use wide::{f32x4, i32x4};

#[inline]
fn load_i32x4(raw: &[u8], width: usize, y: usize, x: usize) -> f32x4 {
    let start = (y * width + x) * 4;
    let samples: [i32; 4] = bytemuck::pod_read_unaligned(&raw[start..start + 16]);
    i32x4::new(samples.map(i32::from_le)).round_float()
}

#[inline]
fn filter_i32x4(
    raw: &[u8],
    width: usize,
    y: usize,
    x: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [i32; 4] {
    let row = |dy: isize, kernel_start: usize| {
        let source_y = (y as isize + dy) as usize;
        let left = load_i32x4(raw, width, source_y, x - 1);
        let middle = load_i32x4(raw, width, source_y, x);
        let right = load_i32x4(raw, width, source_y, x + 1);
        let sum = middle * f32x4::splat(kernel[kernel_start + 1]);
        let sum = left.mul_add(f32x4::splat(kernel[kernel_start]), sum);
        right.mul_add(f32x4::splat(kernel[kernel_start + 2]), sum)
    };
    let mut total = f32x4::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    total
        .simd_ge(f32x4::ZERO)
        .select(total, f32x4::ZERO)
        .trunc_int()
        .to_array()
}

#[inline]
fn filter_i32_scalar(
    raw: &[u8],
    width: usize,
    y: usize,
    x: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> i32 {
    let row = |dy: isize, kernel_start: usize| {
        let source_y = (y as isize + dy) as usize;
        let read = |source_x: usize| -> f32 {
            let start = (source_y * width + source_x) * 4;
            i32::from_le_bytes([raw[start], raw[start + 1], raw[start + 2], raw[start + 3]]) as f32
        };
        let sum = read(x) * kernel[kernel_start + 1];
        let sum = read(x - 1).mul_add(kernel[kernel_start], sum);
        read(x + 1).mul_add(kernel[kernel_start + 2], sum)
    };
    let mut value = rounding_bias;
    value += row(1, 0);
    value += row(0, 3);
    value += row(-1, 6);
    if value >= 0.0 { value as i32 } else { 0 }
}

/// Apply the exact I-mode 3x3 convolution and return block/tail counts.
pub(crate) fn filter_3x3_i32_rows(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    let byte_count = pixel_count.checked_mul(4)?;
    if raw.len() != byte_count || output.len() != byte_count {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }

    let interior_width = width - 2;
    let interior_height = height - 2;
    let vector_blocks = interior_width / 4 * interior_height;
    let scalar_tail = interior_width % 4 * interior_height;
    let row_stride = width * 4;
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        let mut x = 1usize;
        while x + 3 < width - 1 {
            let values = filter_i32x4(raw, width, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate() {
                let start = (x + lane) * 4;
                row[start..start + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += 4;
        }
        while x < width - 1 {
            let value = filter_i32_scalar(raw, width, y, x, kernel, rounding_bias);
            let start = x * 4;
            row[start..start + 4].copy_from_slice(&value.to_le_bytes());
            x += 1;
        }
    }
    Some((vector_blocks as u64, scalar_tail as u64))
}
