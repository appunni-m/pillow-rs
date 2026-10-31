//! NEON implementation of native LA Paste with an L mask.

use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::SourceLayout,
    plan::BytePlan,
    region::checked_row_range,
    vector::blend_l_masked_block8,
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
    if plan.source.mode() != Mode::ImageLumaA8
        || mask_image.mode() != Mode::ImageLuma8
        || plan.source_layout != SourceLayout::Native
        || plan.broadcast_source
        || mask_layout.channels != 1
        || mask_layout.value_index != 0
        || mask_layout.binary
        || mask_layout.premultiplied
        || plan.region.width < 8
        || plan.region.height == 0
    {
        return Ok(None);
    }

    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width = usize::try_from(plan.destination_size.0)
        .map_err(|_| overflow("destination width"))?;
    let source_stride = source_width
        .checked_mul(2)
        .ok_or_else(|| overflow("source stride"))?;
    let destination_stride = destination_width
        .checked_mul(2)
        .ok_or_else(|| overflow("destination stride"))?;
    let mask_stride = usize::try_from(plan.region_size.0).map_err(|_| overflow("mask stride"))?;
    let source = plan.source.as_bytes();
    let mask = mask_image.as_bytes();
    let mut output = plan.destination.to_vec();

    for row in 0..plan.region.height {
        let source_y = plan
            .region
            .source_top
            .checked_add(row)
            .ok_or_else(|| overflow("source row"))?;
        let destination_y = plan
            .region
            .destination_top
            .checked_add(row)
            .ok_or_else(|| overflow("destination row"))?;
        let source_range = checked_row_range(
            source_y,
            plan.region.source_left,
            plan.region.width,
            2,
            source_stride,
        )
        .ok_or_else(|| overflow("source row range"))?;
        let destination_range = checked_row_range(
            destination_y,
            plan.region.destination_left,
            plan.region.width,
            2,
            destination_stride,
        )
        .ok_or_else(|| overflow("destination row range"))?;
        let mask_range = checked_row_range(
            source_y,
            plan.region.source_left,
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

    let row_bytes = u64::from(plan.region.width).saturating_mul(2);
    let height = u64::from(plan.region.height);
    let remainder = row_bytes % 16;
    let tail_blocks = if remainder == 0 {
        0
    } else if remainder <= 8 {
        1
    } else {
        2
    };
    let vector_blocks = (row_bytes / 16)
        .saturating_add(tail_blocks)
        .saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    crate::compute::record_pipeline_operation_path("neon-la-l-blend");
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
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(16)
                .expect("a complete 64-byte group fits the source row"),
        );
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(32)
                .expect("a complete 64-byte group fits the source row"),
        );
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(48)
                .expect("a complete 64-byte group fits the source row"),
        );
    }
    for start in (unrolled_len..vector_len).step_by(16) {
        blend_neon_block(source, destination, mask, start);
    }
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding each length down to an eight-byte block cannot exceed the slice length"
    )]
    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len..vector_len8).step_by(8) {
        let end = start
            .checked_add(8)
            .expect("a complete eight-byte group fits the source row");
        let source_block = <[u8; 8]>::try_from(&source[start..end])
            .expect("validated LA Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..end])
            .expect("validated LA Paste row has a complete 8-byte destination block");
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the complete LA block maps within its validated L-mask row"
        )]
        let mask_block = std::array::from_fn(|lane| mask[start / 2 + lane / 2]);
        let blended = blend_l_masked_block8(source_block, destination_block, mask_block);
        destination[start..end].copy_from_slice(&blended);
    }

    let tail = source.len().saturating_sub(vector_len8);
    if tail != 0 {
        let mut source_block = [0u8; 8];
        let mut destination_block = [0u8; 8];
        source_block[..tail].copy_from_slice(&source[vector_len8..]);
        destination_block[..tail].copy_from_slice(&destination[vector_len8..]);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "active tail lanes map within the validated L-mask row"
        )]
        let mask_block = std::array::from_fn(|lane| {
            if lane < tail {
                mask[vector_len8 / 2 + lane / 2]
            } else {
                0
            }
        });
        let blended = blend_l_masked_block8(source_block, destination_block, mask_block);
        destination[vector_len8..].copy_from_slice(&blended[..tail]);
    }
}

#[allow(unsafe_code)]
#[inline]
fn blend_neon_block(source: &[u8], destination: &mut [u8], mask: &[u8], start: usize) {
    use core::arch::aarch64 as neon;

    macro_rules! blend {
        ($source:expr, $destination:expr, $mask:expr) => {{
            let inverse = neon::vsub_u8(neon::vdup_n_u8(255), $mask);
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, inverse);
            let incremented = neon::vaddq_u16(weighted, neon::vdupq_n_u16(128));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }

    let mask_start = start / 2;
    // SAFETY: the caller bounds `start` to a complete 16-byte LA source and
    // destination block, corresponding to eight L-mask bytes. NEON's
    // interleaved byte loads/stores permit unaligned pointers; the mutable
    // destination is disjoint from the immutable source and mask.
    unsafe {
        let source_pair = neon::vld2_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination_pair = neon::vld2_u8(destination_ptr);
        let mask = neon::vld1_u8(mask.as_ptr().add(mask_start));
        let output = neon::uint8x8x2_t(
            blend!(source_pair.0, destination_pair.0, mask),
            blend!(source_pair.1, destination_pair.1, mask),
        );
        neon::vst2_u8(destination_ptr, output);
    }
}

fn invalid_shape(part: &'static str) -> PilError {
    PilError::InternalError(format!("NEON Paste {part} row did not match its plan"))
}

fn overflow(part: &'static str) -> PilError {
    PilError::OverflowError(format!("NEON Paste {part} overflowed"))
}
