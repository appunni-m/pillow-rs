//! NEON implementation of native L Paste with an L mask.

use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::SourceLayout,
    plan::BytePlan,
    region::checked_row_range,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(super) fn try_execute(plan: &BytePlan<'_>) -> Result<Option<Vec<u8>>, PilError> {
    let Some(mask_image) = plan.mask.as_deref() else {
        return Ok(None);
    };
    let Some(mask_layout) = plan.mask_layout else {
        return Ok(None);
    };
    if plan.source.mode() != Mode::ImageLuma8
        || mask_image.mode() != Mode::ImageLuma8
        || plan.source_layout != SourceLayout::Native
        || plan.broadcast_source
        || mask_layout.channels != 1
        || mask_layout.value_index != 0
        || mask_layout.binary
        || mask_layout.premultiplied
        || plan.region.width < 16
        || plan.region.height == 0
    {
        return Ok(None);
    }

    let source = plan.source.as_bytes();
    let mask = mask_image.as_bytes();
    let source_stride = usize::try_from(plan.source_size.0)
        .map_err(|_| overflow("source stride"))?;
    let destination_stride = usize::try_from(plan.destination_size.0)
        .map_err(|_| overflow("destination stride"))?;
    let mask_stride = usize::try_from(plan.region_size.0)
        .map_err(|_| overflow("mask stride"))?;
    let source_left = plan.region.source_left;
    let destination_left = plan.region.destination_left;
    let source_top = plan.region.source_top;
    let destination_top = plan.region.destination_top;
    let mut output = plan.destination.to_vec();

    for row in 0..plan.region.height {
        let source_y = source_top
            .checked_add(row)
            .ok_or_else(|| overflow("source row"))?;
        let destination_y = destination_top
            .checked_add(row)
            .ok_or_else(|| overflow("destination row"))?;
        let source_range = checked_row_range(
            source_y,
            source_left,
            plan.region.width,
            1,
            source_stride,
        )
        .ok_or_else(|| overflow("source row range"))?;
        let destination_range = checked_row_range(
            destination_y,
            destination_left,
            plan.region.width,
            1,
            destination_stride,
        )
        .ok_or_else(|| overflow("destination row range"))?;
        let mask_range = checked_row_range(
            source_y,
            source_left,
            plan.region.width,
            1,
            mask_stride,
        )
        .ok_or_else(|| overflow("mask row range"))?;
        let source_row = source
            .get(source_range)
            .ok_or_else(|| invalid_shape("source"))?;
        let destination_row = output
            .get_mut(destination_range)
            .ok_or_else(|| invalid_shape("destination"))?;
        let mask_row = mask.get(mask_range).ok_or_else(|| invalid_shape("mask"))?;
        blend_row(source_row, destination_row, mask_row);
    }

    let height = u64::from(plan.region.height);
    let vector_blocks = u64::from(plan.region.width / 16).saturating_mul(height);
    let scalar_tail = u64::from(plan.region.width % 16).saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("neon-blend");
    Ok(Some(output))
}

fn blend_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding each length down to a block cannot exceed the slice length"
    )]
    let vector_len = source.len() / 16 * 16;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding the vector prefix down to four blocks cannot exceed its length"
    )]
    let unrolled_len = vector_len / 64 * 64;
    for start in (0..unrolled_len).step_by(64) {
        blend_neon_block(source, destination, mask, start);
        let second = start
            .checked_add(16)
            .expect("a complete 64-byte group fits the source row");
        let third = start
            .checked_add(32)
            .expect("a complete 64-byte group fits the source row");
        let fourth = start
            .checked_add(48)
            .expect("a complete 64-byte group fits the source row");
        blend_neon_block(source, destination, mask, second);
        blend_neon_block(source, destination, mask, third);
        blend_neon_block(source, destination, mask, fourth);
    }
    for start in (unrolled_len..vector_len).step_by(16) {
        blend_neon_block(source, destination, mask, start);
    }
    for index in vector_len..source.len() {
        let weight = u16::from(mask[index]);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the byte mask is at most 255"
        )]
        let inverse = 255 - weight;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
        )]
        let weighted = u16::from(source[index]) * weight
            + u16::from(destination[index]) * inverse
            + 127;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "DIV255 yields an 8-bit convex blend of byte channels"
        )]
        destination[index] = (weighted / 255) as u8;
    }
}

#[allow(unsafe_code)]
#[inline]
fn blend_neon_block(source: &[u8], destination: &mut [u8], mask: &[u8], start: usize) {
    use core::arch::aarch64 as neon;

    macro_rules! blend_half {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, $inverse);
            let rounded = neon::vaddq_u16(weighted, neon::vdupq_n_u16(127));
            let incremented = neon::vaddq_u16(rounded, neon::vdupq_n_u16(1));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }

    // SAFETY: the caller restricts `start` to a complete 16-byte prefix of
    // equally sized source, destination, and mask rows. NEON loads and stores
    // allow unaligned addresses; the destination is disjoint from the inputs.
    unsafe {
        let source = neon::vld1q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination = neon::vld1q_u8(destination_ptr);
        let mask = neon::vld1q_u8(mask.as_ptr().add(start));
        let inverse = neon::vsubq_u8(neon::vdupq_n_u8(255), mask);
        let output = neon::vcombine_u8(
            blend_half!(
                neon::vget_low_u8(source),
                neon::vget_low_u8(destination),
                neon::vget_low_u8(mask),
                neon::vget_low_u8(inverse)
            ),
            blend_half!(
                neon::vget_high_u8(source),
                neon::vget_high_u8(destination),
                neon::vget_high_u8(mask),
                neon::vget_high_u8(inverse)
            ),
        );
        neon::vst1q_u8(destination_ptr, output);
    }
}

fn invalid_shape(part: &'static str) -> PilError {
    PilError::InternalError(format!("NEON Paste {part} row did not match its plan"))
}

fn overflow(part: &'static str) -> PilError {
    PilError::OverflowError(format!("NEON Paste {part} overflowed"))
}
