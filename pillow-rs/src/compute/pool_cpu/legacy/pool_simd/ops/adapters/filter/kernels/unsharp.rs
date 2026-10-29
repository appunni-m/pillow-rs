//! Native RGB SIMD blend for Pillow UnsharpMask.

use super::super::super::simd_pack_u16x16;
use wide::{u8x16, u16x16};

/// Apply Pillow's exact 150% UnsharpMask blend to packed native RGB bytes.
///
/// At 150%, `diff * percent / 100` is truncating `diff * 3 / 2`. Computing
/// the magnitude first makes the signed truncation exact with unsigned lanes:
/// `sign(diff) * floor(abs(diff) * 3 / 2)`. The input is interleaved RGB, but
/// the operation is independent per byte and therefore needs no channel
/// conversion or shuffle.
pub(crate) fn simd_unsharp_blend_rgb_150(
    original: &[u8],
    blurred_output: &mut [u8],
    threshold: i32,
) -> bool {
    if original.len() != blurred_output.len() {
        return false;
    }

    let mut original_chunks = original.chunks_exact(16);
    let mut output_chunks = blurred_output.chunks_exact_mut(16);
    for output_chunk in output_chunks.by_ref() {
        let Some(original_chunk) = original_chunks.next() else {
            return false;
        };
        let Ok(original_lanes) = <[u8; 16]>::try_from(original_chunk) else {
            return false;
        };
        let Ok(blurred_lanes) = <[u8; 16]>::try_from(&*output_chunk) else {
            return false;
        };
        let original_lanes = u16x16::from(u8x16::new(original_lanes));
        let blurred_lanes = u16x16::from(u8x16::new(blurred_lanes));
        let original_is_greater = original_lanes.simd_ge(blurred_lanes);
        let distance = original_is_greater.select(
            original_lanes - blurred_lanes,
            blurred_lanes - original_lanes,
        );
        let adjustment = (distance * u16x16::splat(3)) >> 1u32;
        // Preserve the signed subtraction through the u16 -> i16 reinterpret
        // in `simd_pack_u16x16`; its saturating narrow clamps both underflow
        // and overflow to the byte range. The adjustment is at most 382, so
        // wrapped negative lanes remain within i16's signed range.
        let sharpened =
            original_is_greater.select(original_lanes + adjustment, original_lanes - adjustment);
        let result = if threshold < 0 {
            sharpened
        } else {
            distance
                .simd_gt(u16x16::splat(threshold.min(255) as u16))
                .select(sharpened, original_lanes)
        };
        output_chunk.copy_from_slice(&simd_pack_u16x16(result).to_array());
    }

    for (original, output) in original_chunks
        .remainder()
        .iter()
        .zip(output_chunks.into_remainder())
    {
        let blurred = *output;
        let difference = i32::from(*original) - i32::from(blurred);
        let sharpened = if difference.abs() > threshold {
            let adjustment = difference * 150 / 100;
            (i32::from(*original) + adjustment).clamp(0, 255) as u8
        } else {
            *original
        };
        *output = sharpened;
    }

    true
}
