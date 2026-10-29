//! Native-format radius-one Gaussian and BoxBlur kernels.

use super::{
    SIMD_BOX_BLUR_BIAS, simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes, u16x16, u32x16,
};
use wide::u8x16;
/// Apply one RGBA vertical radius-one box pass directly in row-major storage.
///
/// GaussianBlur's three box passes use the same vertical sample on each of
/// the four native RGBA bytes. Process complete packed pixels in 16-byte
/// vectors and retain Pillow's fixed-point weights and byte rounding. This
/// avoids transposing the full RGBA frame just to make columns contiguous.
#[inline(always)]
fn simd_rgba_blur_scalar_pixel(
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
    for channel in 0..4 {
        let sum = u32::from(source[left * 4 + channel])
            + u32::from(source[x * 4 + channel])
            + u32::from(source[right * 4 + channel]);
        let edge =
            u32::from(source[far_left * 4 + channel]) + u32::from(source[far_right * 4 + channel]);
        let weighted = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(edge.wrapping_mul(fractional_weight))
            .wrapping_add(SIMD_BOX_BLUR_BIAS);
        destination[x * 4 + channel] = (weighted >> 24) as u8;
    }
}

/// Apply one RGBA horizontal radius-one box pass with direct vector loads.
///
/// Four neighboring pixels occupy one 16-byte vector. The interior therefore
/// uses aligned channel correspondence for all four RGBA bytes, while the
/// two-pixel borders and the final incomplete group use the exact clamped
/// scalar formula. This removes the per-pixel rolling-recurrence setup from
/// the three horizontal Gaussian box passes.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_rgba_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(4)
        .expect("validated RGBA dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let interior_end = width.saturating_sub(2);
    let vector_blocks_per_row = interior_end.saturating_sub(2) / 4;
    let vector_blocks = vector_blocks_per_row.saturating_mul(height) as u64;
    let scalar_tail =
        (width.saturating_sub(vector_blocks_per_row * 4)).saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }

    for y in 0..height {
        let row_start = y * row_stride;
        let input = &source[row_start..row_start + row_stride];
        let output = &mut destination[row_start..row_start + row_stride];
        let prefix_end = width.min(2);
        for x in 0..prefix_end {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }

        let mut x = 2usize;
        while x + 4 <= interior_end {
            let byte_start = x * 4;
            let load = |pixel: usize| {
                let start = pixel * 4;
                u32x16::from(u16x16::from(simd_blur_load_u8x16(input, start)))
            };
            let sum = load(x - 1) + load(x) + load(x + 1);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(x - 2) + load(x + 2)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_start..byte_start + 16].copy_from_slice(&packed);
            x += 4;
        }

        for x in x..interior_end {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }
        for x in interior_end.max(prefix_end)..width {
            simd_rgba_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
        }
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn simd_rgba_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(4)
        .expect("validated RGBA dimensions fit the native row stride");
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
