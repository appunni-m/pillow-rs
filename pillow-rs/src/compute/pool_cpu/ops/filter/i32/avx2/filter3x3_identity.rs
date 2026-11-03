//! AVX2-selected uniform-image proof for I-mode 3x3 convolution.

use wide::u8x16;

/// Scan packed I samples in wide blocks, then apply the exact shared Pillow-order proof.
pub(in crate::compute::pool_cpu::ops::filter::i32) fn uniform_i32_filter3x3_is_identity(
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
        || pixel_count > super::super::default::I32_UNIFORM_FILTER3X3_MAX_PIXELS
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
    super::super::default::uniform_i32_filter3x3_sample_is_identity(
        sample_i32,
        kernel,
        rounding_bias,
    )
}
