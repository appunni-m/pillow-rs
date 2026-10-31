//! Portable-wide implementation of Pillow's native RGB 150% UnsharpMask blend.

use wide::{u8x16, u16x16};

/// Apply the exact signed-truncating 150% blend to interleaved RGB bytes.
pub(super) fn execute_unsharp_rgb150(
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
        // The wrapped negative lanes are in i16 range; the shared saturating
        // pack restores Pillow's zero/255 clipping at the output boundary.
        let sharpened =
            original_is_greater.select(original_lanes + adjustment, original_lanes - adjustment);
        let result = if threshold < 0 {
            sharpened
        } else {
            distance
                .simd_gt(u16x16::splat(threshold.min(255) as u16))
                .select(sharpened, original_lanes)
        };
        output_chunk.copy_from_slice(
            &crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(result).to_array(),
        );
    }

    for (original, output) in original_chunks
        .remainder()
        .iter()
        .zip(output_chunks.into_remainder())
    {
        let blurred = *output;
        let difference = i32::from(*original) - i32::from(blurred);
        *output = if difference.abs() > threshold {
            (i32::from(*original) + difference * 3 / 2).clamp(0, 255) as u8
        } else {
            *original
        };
    }

    true
}
