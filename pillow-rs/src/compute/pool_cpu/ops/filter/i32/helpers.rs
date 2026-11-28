//! Scalar I-mode convolution tails shared by architecture strategies.

use crate::compute::pool_cpu::ops::filter::helpers::pillow_kernel_row_3;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::compute::pool_cpu::ops::filter::helpers::pillow_kernel_row_5;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(super) fn filter_i32_3x3_scalar(
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
        pillow_kernel_row_3(
            [read(x - 1), read(x), read(x + 1)],
            &kernel[kernel_start..kernel_start + 3],
        )
    };
    let mut value = rounding_bias;
    value += row(1, 0);
    value += row(0, 3);
    value += row(-1, 6);
    if value >= 0.0 { value as i32 } else { 0 }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(super) fn filter_i32_5x5_scalar(
    raw: &[u8],
    width: usize,
    y: usize,
    x: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> i32 {
    let row = |dy: isize, kernel_start: usize| {
        let source_y = (y as isize + dy) as usize;
        let pixels = std::array::from_fn(|tap| {
            let source_x = x + tap - 2;
            let start = (source_y * width + source_x) * 4;
            i32::from_le_bytes([raw[start], raw[start + 1], raw[start + 2], raw[start + 3]]) as f32
        });
        pillow_kernel_row_5(pixels, &kernel[kernel_start..kernel_start + 5])
    };
    let mut value = rounding_bias;
    value += row(2, 0);
    value += row(1, 5);
    value += row(0, 10);
    value += row(-1, 15);
    value += row(-2, 20);
    if value >= 0.0 { value as i32 } else { 0 }
}
