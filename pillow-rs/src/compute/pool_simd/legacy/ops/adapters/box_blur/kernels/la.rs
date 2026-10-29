//! Native-format radius-one Gaussian and BoxBlur kernels.

use super::{
    SIMD_BOX_BLUR_BIAS, simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes, u16x16, u32x16,
};
use wide::u8x16;
#[inline(always)]
pub(in crate::compute::pool_simd::ops::adapters) fn simd_la_blur_scalar_pixel(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    x: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let center = x * 2;
    for channel in 0..2 {
        let sum = u32::from(source[left * 2 + channel])
            + u32::from(source[center + channel])
            + u32::from(source[right * 2 + channel]);
        let edge =
            u32::from(source[far_left * 2 + channel]) + u32::from(source[far_right * 2 + channel]);
        let weighted = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(edge.wrapping_mul(fractional_weight))
            .wrapping_add(SIMD_BOX_BLUR_BIAS);
        destination[center + channel] = (weighted >> 24) as u8;
    }
}

/// Apply one native-LA vertical radius-one box pass directly in row-major
/// storage. Eight interleaved LA pixels fit each vector; border rows and an
/// incomplete final vector retain Pillow's clamped-edge and byte rounding.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_la_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(2)
        .expect("validated LA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    for y in 0..height {
        let sum_top_base = y.saturating_sub(1) * row_stride;
        let sum_middle_base = y * row_stride;
        let sum_bottom_base = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top_base = y.saturating_sub(2) * row_stride;
        let edge_bottom_base = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;

        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |base: usize| {
                if count == 16 {
                    u32x16::from(u16x16::from(simd_blur_load_u8x16(source, base + x)))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&source[base + x..base + x + count]);
                    u32x16::from(u16x16::from(u8x16::new(padded)))
                }
            };
            let sum = load(sum_top_base) + load(sum_middle_base) + load(sum_bottom_base);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top_base) + load(edge_bottom_base)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
}
