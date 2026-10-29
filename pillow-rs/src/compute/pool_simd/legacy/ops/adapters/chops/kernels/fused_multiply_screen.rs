//! SIMD row kernel for the fused Multiply→Screen path.

use super::{simd_div255, simd_pack_u16x16};
use wide::{u8x16, u16x16};

#[inline]
pub(in crate::compute::pool_simd::ops::adapters::chops) fn simd_fused_multiply_screen_row(
    left_bytes: &[u8],
    right_bytes: &[u8],
    output: &mut [u8],
) -> bool {
    if left_bytes.len() != right_bytes.len() || left_bytes.len() != output.len() {
        return false;
    }

    let mut left_chunks = left_bytes.chunks_exact(16);
    let mut right_chunks = right_bytes.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((left_chunk, right_chunk), output_chunk) in left_chunks
        .by_ref()
        .zip(right_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        let Ok(left) = <[u8; 16]>::try_from(left_chunk) else {
            return false;
        };
        let Ok(right) = <[u8; 16]>::try_from(right_chunk) else {
            return false;
        };
        let left = u16x16::from(u8x16::new(left));
        let right = u16x16::from(u8x16::new(right));
        let multiplied = simd_div255(left * right);
        let result = multiplied + right - simd_div255(multiplied * right);
        output_chunk.copy_from_slice(&simd_pack_u16x16(result).to_array());
    }
    for ((&left, &right), output) in left_chunks
        .remainder()
        .iter()
        .zip(right_chunks.remainder())
        .zip(output_chunks.into_remainder())
    {
        let multiplied = (left as u32 * right as u32 / 255) as u8;
        *output = (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
    }
    true
}
