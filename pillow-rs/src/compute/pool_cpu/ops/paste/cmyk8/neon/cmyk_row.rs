//! NEON implementation of native CMYK Paste with an L mask.

use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::SourceLayout,
    plan::BytePlan,
    region::checked_row_range,
    vector::{blend_l_masked_block8, blend_l_masked_block16},
};
use crate::error::PilError;
use crate::raster::Mode;

pub(super) fn try_execute<I>(plan: &BytePlan<'_, I>) -> Result<Option<Vec<u8>>, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    if !crate::compute::pool_cpu::ops::paste::helpers::strategy::l_mask_neon_selected(
        &plan.selection,
        Mode::ImageCmyk8,
        4,
        16,
    ) {
        return Ok(None);
    }
    let Some(mask_image) = plan.mask.as_deref() else {
        return Ok(None);
    };
    let Some(mask_layout) = plan.mask_layout else {
        return Ok(None);
    };
    if plan.source.mode() != Mode::ImageCmyk8
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

    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width = usize::try_from(plan.destination_size.0)
        .map_err(|_| overflow("destination width"))?;
    let source_stride = source_width
        .checked_mul(4)
        .ok_or_else(|| overflow("source stride"))?;
    let destination_stride = destination_width
        .checked_mul(4)
        .ok_or_else(|| overflow("destination stride"))?;
    let mask_stride = usize::try_from(plan.region_size.0).map_err(|_| overflow("mask stride"))?;
    let source = plan.source.as_bytes();
    let mask = mask_image.as_bytes();
    let mut output = plan.initialize_destination()?;

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
            4,
            source_stride,
        )
        .ok_or_else(|| overflow("source row range"))?;
        let destination_range = checked_row_range(
            destination_y,
            plan.region.destination_left,
            plan.region.width,
            4,
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

    let row_bytes = u64::from(plan.region.width).saturating_mul(4);
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
        .saturating_mul(u64::from(plan.region.height));
    let scalar_tail = (row_bytes % 8).saturating_mul(u64::from(plan.region.height));
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("neon-cmyk-l-blend");
    Ok(Some(output))
}

fn blend_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding to a complete 64-byte CMYK block cannot exceed the row length"
    )]
    let neon_len = source.len() / 64 * 64;
    blend_neon_prefix(source, destination, mask, neon_len);
    blend_wide_tail(source, destination, mask, neon_len);
}

fn blend_neon_prefix(source: &[u8], destination: &mut [u8], mask: &[u8], neon_len: usize) {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding the NEON prefix down to four 64-byte blocks cannot exceed its length"
    )]
    let unrolled_len = neon_len / 256 * 256;
    for start in (0..unrolled_len).step_by(256) {
        blend_neon_block(source, destination, mask, start);
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(64)
                .expect("a complete 256-byte group fits the source row"),
        );
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(128)
                .expect("a complete 256-byte group fits the source row"),
        );
        blend_neon_block(
            source,
            destination,
            mask,
            start
                .checked_add(192)
                .expect("a complete 256-byte group fits the source row"),
        );
    }
    for start in (unrolled_len..neon_len).step_by(64) {
        blend_neon_block(source, destination, mask, start);
    }
}

fn blend_wide_tail(source: &[u8], destination: &mut [u8], mask: &[u8], neon_len: usize) {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding the row length down to a 16-byte block cannot exceed it"
    )]
    let vector_len16 = source.len() / 16 * 16;
    for start in (neon_len..vector_len16).step_by(16) {
        let end = start
            .checked_add(16)
            .expect("a complete 16-byte CMYK block fits the source row");
        let source_block = <[u8; 16]>::try_from(&source[start..end])
            .expect("validated CMYK Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..end])
            .expect("validated CMYK Paste row has a complete 16-byte destination block");
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "complete block lanes map within four validated L-mask pixels"
        )]
        let mask_block = std::array::from_fn(|lane| mask[(start + lane) / 4]);
        let blended = blend_l_masked_block16(source_block, destination_block, mask_block);
        destination[start..end].copy_from_slice(&blended);
    }

    #[expect(
        clippy::arithmetic_side_effects,
        reason = "rounding the row length down to an eight-byte block cannot exceed it"
    )]
    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16.max(neon_len)..vector_len8).step_by(8) {
        let end = start
            .checked_add(8)
            .expect("a complete 8-byte CMYK block fits the source row");
        let source_block = <[u8; 8]>::try_from(&source[start..end])
            .expect("validated CMYK Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..end])
            .expect("validated CMYK Paste row has a complete 8-byte destination block");
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "complete block lanes map within two validated L-mask pixels"
        )]
        let mask_block = [
            mask[start / 4],
            mask[start / 4],
            mask[start / 4],
            mask[start / 4],
            mask[start / 4 + 1],
            mask[start / 4 + 1],
            mask[start / 4 + 1],
            mask[start / 4 + 1],
        ];
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
                mask[(vector_len8 + lane) / 4]
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

    macro_rules! blend_half {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            let weighted = neon::vmlal_u8(neon::vmull_u8($source, $mask), $destination, $inverse);
            let incremented = neon::vaddq_u16(weighted, neon::vdupq_n_u16(128));
            let quotient = neon::vshrq_n_u16(
                neon::vaddq_u16(incremented, neon::vshrq_n_u16(incremented, 8)),
                8,
            );
            neon::vmovn_u16(quotient)
        }};
    }
    macro_rules! blend_channel {
        ($source:expr, $destination:expr, $mask:expr, $inverse:expr) => {{
            neon::vcombine_u8(
                blend_half!(
                    neon::vget_low_u8($source),
                    neon::vget_low_u8($destination),
                    neon::vget_low_u8($mask),
                    neon::vget_low_u8($inverse)
                ),
                blend_half!(
                    neon::vget_high_u8($source),
                    neon::vget_high_u8($destination),
                    neon::vget_high_u8($mask),
                    neon::vget_high_u8($inverse)
                ),
            )
        }};
    }

    // SAFETY: callers limit `start` to a complete 64-byte CMYK block and its
    // sixteen L-mask samples. Interleaved NEON access permits unaligned
    // pointers; safe borrows keep destination disjoint from both inputs.
    unsafe {
        let source_pixels = neon::vld4q_u8(source.as_ptr().add(start));
        let destination_ptr = destination.as_mut_ptr().add(start);
        let destination_pixels = neon::vld4q_u8(destination_ptr);
        let mask = neon::vld1q_u8(mask.as_ptr().add(start / 4));
        let inverse = neon::vsubq_u8(neon::vdupq_n_u8(255), mask);
        let output = neon::uint8x16x4_t(
            blend_channel!(source_pixels.0, destination_pixels.0, mask, inverse),
            blend_channel!(source_pixels.1, destination_pixels.1, mask, inverse),
            blend_channel!(source_pixels.2, destination_pixels.2, mask, inverse),
            blend_channel!(source_pixels.3, destination_pixels.3, mask, inverse),
        );
        neon::vst4q_u8(destination_ptr, output);
    }
}

fn invalid_shape(part: &'static str) -> PilError {
    PilError::InternalError(format!("NEON Paste {part} row did not match its plan"))
}

fn overflow(part: &'static str) -> PilError {
    PilError::OverflowError(format!("NEON Paste {part} overflowed"))
}
