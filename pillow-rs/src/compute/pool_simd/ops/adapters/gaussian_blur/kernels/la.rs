//! Native-LA horizontal radius-one SIMD row kernel.

use super::super::{
    SIMD_BOX_BLUR_BIAS, simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes,
    simd_la_blur_scalar_pixel, u16x16, u32x16,
};

/// Apply one horizontal LA box pass to a native `[L, A]` row.
#[inline(always)]
pub(super) fn horizontal_radius_one(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert_eq!(input.len(), width.saturating_mul(2));
    debug_assert_eq!(output.len(), input.len());

    let interior_end = width.saturating_sub(2);
    let prefix_end = width.min(2);
    for x in 0..prefix_end {
        simd_la_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
    }

    let mut x = 2usize;
    while x + 8 <= interior_end {
        let byte_start = x * 2;
        let load =
            |pixel: usize| u32x16::from(u16x16::from(simd_blur_load_u8x16(input, pixel * 2)));
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
        x += 8;
    }

    for x in x..interior_end {
        simd_la_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
    }
    for x in interior_end.max(prefix_end)..width {
        simd_la_blur_scalar_pixel(input, output, width, x, whole_weight, fractional_weight);
    }
}
