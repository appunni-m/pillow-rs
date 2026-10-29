//! Typed I-mode 3x3 filter kernels.

use wide::{f32x4, i32x4, u8x16};

/// Load four adjacent signed I-mode samples and convert them to float lanes.
#[inline]
fn native_filter_3x3_i32_load4(raw: &[u8], width: usize, y: usize, x: usize) -> f32x4 {
    let base = (y * width + x) * 4;
    let samples: [i32; 4] = bytemuck::pod_read_unaligned(&raw[base..base + 16]);
    i32x4::new(samples.map(i32::from_le)).round_float()
}

/// Evaluate four I-mode samples with overlapping contiguous loads and SIMD lanes.
#[inline]
fn native_filter_3x3_i32_vector4(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [i32; 4] {
    let row = |dy: isize, kernel_start: usize| -> f32x4 {
        let source_y = (y as isize + dy) as usize;
        let left = native_filter_3x3_i32_load4(raw, width, source_y, x_start - 1);
        let middle = native_filter_3x3_i32_load4(raw, width, source_y, x_start);
        let right = native_filter_3x3_i32_load4(raw, width, source_y, x_start + 1);
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
fn native_filter_3x3_i32_scalar(
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
            let base = (source_y * width + source_x) * 4;
            i32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]) as f32
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

/// SIMD scan for a uniform I-mode buffer before the exact identity proof.
/// Compare four packed I samples per vector; leave only the final partial
/// vector to scalar byte checks, then share the CPU's Pillow-order f32 proof.
pub(crate) fn uniform_i32_filter3x3_is_identity_simd(
    raw: &[u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> bool {
    if width < 3 || height < 3 {
        return false;
    }
    let Some(pixel_count) = width.checked_mul(height) else {
        return false;
    };
    if pixel_count == 0
        || pixel_count > crate::compute::pool_cpu::ops::filter::I32_UNIFORM_FILTER3X3_MAX_PIXELS
    {
        return false;
    }
    let Some(expected_len) = pixel_count.checked_mul(4) else {
        return false;
    };
    if raw.len() != expected_len {
        return false;
    }

    let sample_bytes = &raw[..4];
    let pattern = std::array::from_fn(|index| sample_bytes[index % 4]);
    let pattern = u8x16::new(pattern);
    let vector_len = raw.len() / 16 * 16;
    for block in raw[..vector_len].chunks_exact(16) {
        let Ok(bytes) = <[u8; 16]>::try_from(block) else {
            return false;
        };
        if !u8x16::new(bytes).simd_eq(pattern).all() {
            return false;
        }
    }
    if !raw[vector_len..]
        .iter()
        .enumerate()
        .all(|(index, &byte)| byte == sample_bytes[index % 4])
    {
        return false;
    }

    let sample_i32 = i32::from_le_bytes([
        sample_bytes[0],
        sample_bytes[1],
        sample_bytes[2],
        sample_bytes[3],
    ]);
    crate::compute::pool_cpu::ops::filter::uniform_i32_filter3x3_sample_is_identity(
        sample_i32,
        kernel,
        rounding_bias,
    )
}

pub(in crate::compute::pool_simd::ops::adapters::filter) fn native_filter_3x3_i32_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> (u64, u64) {
    if width < 3 || height < 3 {
        return (0, 0);
    }
    let interior_width = width - 2;
    let interior_height = height - 2;
    let vector_blocks = interior_width / 4 * interior_height;
    let scalar_tail = interior_width % 4 * interior_height;
    let row_stride = width * 4;
    let apply_row = |y: usize, destination: &mut [u8]| {
        let mut x = 1usize;
        while x + 3 < width - 1 {
            let values = native_filter_3x3_i32_vector4(raw, width, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate() {
                let base = (x + lane) * 4;
                destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += 4;
        }
        while x < width - 1 {
            let value = native_filter_3x3_i32_scalar(raw, width, y, x, kernel, rounding_bias);
            let base = x * 4;
            destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            x += 1;
        }
    };

    #[cfg(feature = "parallel")]
    super::super::super::simd_rows_mut_serial!(
        out,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            if (1..height - 1).contains(&y) {
                apply_row(y, row);
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
    (vector_blocks as u64, scalar_tail as u64)
}
