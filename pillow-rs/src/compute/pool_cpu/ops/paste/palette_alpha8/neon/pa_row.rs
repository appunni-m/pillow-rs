//! Exact PA L-mask row strategy selected by the NEON operation contract.

use std::ops::Deref;

use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::{MaskLayout, SourceLayout},
    plan::BytePlan,
    region::checked_row_range,
    vector::{blend_l_masked_block8, blend_l_masked_block16},
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn execute<I>(plan: BytePlan<'_, I>) -> Result<Vec<u8>, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let source = plan.source.deref();
    let mask = plan
        .mask
        .as_deref()
        .ok_or_else(|| PilError::InternalError("PA NEON Paste strategy lost its L mask".into()))?;
    if source.mode() != Mode::ImagePaletteAlpha8
        || plan.source_layout != SourceLayout::Native
        || plan.broadcast_source
        || plan.mask_layout != Some(luma_mask_layout())
    {
        return Err(PilError::InternalError(
            "PA NEON Paste strategy received a different input layout".into(),
        ));
    }

    let mut output = plan.initialize_destination()?;
    if plan.region.is_empty() {
        crate::compute::record_pipeline_operation_path("vector");
        return Ok(output);
    }

    let source_stride = checked_stride(plan.source_size.0, 2, "source")?;
    let destination_stride = checked_stride(plan.destination_size.0, 2, "destination")?;
    let mask_stride = checked_stride(plan.region_size.0, 1, "mask")?;
    let mut vector_blocks = 0u64;
    for row in 0..plan.region.height {
        let source_y = plan.region.source_top + row;
        let destination_y = plan.region.destination_top + row;
        let source_range = checked_row_range(
            source_y,
            plan.region.source_left,
            plan.region.width,
            2,
            source_stride,
        )
        .ok_or_else(|| invalid_row("source"))?;
        let destination_range = checked_row_range(
            destination_y,
            plan.region.destination_left,
            plan.region.width,
            2,
            destination_stride,
        )
        .ok_or_else(|| invalid_row("destination"))?;
        let mask_range = checked_row_range(
            source_y,
            plan.region.source_left,
            plan.region.width,
            1,
            mask_stride,
        )
        .ok_or_else(|| invalid_row("mask"))?;
        let source_row = source
            .as_bytes()
            .get(source_range)
            .ok_or_else(|| invalid_row("source"))?;
        let mask_row = mask
            .as_bytes()
            .get(mask_range)
            .ok_or_else(|| invalid_row("mask"))?;
        let destination_row = output
            .get_mut(destination_range)
            .ok_or_else(|| invalid_row("destination"))?;
        vector_blocks =
            vector_blocks.saturating_add(blend_pa_row(source_row, destination_row, mask_row));
    }

    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn blend_pa_row(source: &[u8], destination: &mut [u8], mask: &[u8]) -> u64 {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 2, 0);
    debug_assert_eq!(source.len() / 2, mask.len());

    let vector_len16 = source.len() / 16 * 16;
    let mut blocks = 0u64;
    for start in (0..vector_len16).step_by(16) {
        let source_block = <[u8; 16]>::try_from(&source[start..start + 16])
            .expect("PA Paste row has a complete 16-byte block");
        let destination_block = <[u8; 16]>::try_from(&destination[start..start + 16])
            .expect("PA Paste destination row has a complete 16-byte block");
        let mask_block = expand_mask::<16>(mask, start, 16);
        let blended = blend_l_masked_block16(source_block, destination_block, mask_block);
        destination[start..start + 16].copy_from_slice(&blended);
        blocks = blocks.saturating_add(1);
    }

    let vector_len8 = source.len() / 8 * 8;
    for start in (vector_len16..vector_len8).step_by(8) {
        let source_block = <[u8; 8]>::try_from(&source[start..start + 8])
            .expect("PA Paste row has a complete 8-byte block");
        let destination_block = <[u8; 8]>::try_from(&destination[start..start + 8])
            .expect("PA Paste destination row has a complete 8-byte block");
        let mask_block = expand_mask::<8>(mask, start, 8);
        let blended = blend_l_masked_block8(source_block, destination_block, mask_block);
        destination[start..start + 8].copy_from_slice(&blended);
        blocks = blocks.saturating_add(1);
    }

    blocks.saturating_add(blend_pa_tail(source, destination, mask, vector_len8))
}

fn blend_pa_tail(source: &[u8], destination: &mut [u8], mask: &[u8], start: usize) -> u64 {
    let tail = source.len() - start;
    if tail == 0 {
        return 0;
    }
    let mut source_block = [0; 8];
    let mut destination_block = [0; 8];
    source_block[..tail].copy_from_slice(&source[start..]);
    destination_block[..tail].copy_from_slice(&destination[start..]);
    let mask_block = expand_mask::<8>(mask, start, tail);
    let blended = blend_l_masked_block8(source_block, destination_block, mask_block);
    destination[start..].copy_from_slice(&blended[..tail]);
    1
}

fn expand_mask<const LANES: usize>(mask: &[u8], byte_start: usize, active: usize) -> [u8; LANES] {
    std::array::from_fn(|lane| {
        if lane < active {
            mask[byte_start / 2 + lane / 2]
        } else {
            0
        }
    })
}

const fn luma_mask_layout() -> MaskLayout {
    MaskLayout {
        channels: 1,
        value_index: 0,
        binary: false,
        premultiplied: false,
    }
}

fn checked_stride(width: u32, channels: usize, role: &str) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(channels))
        .ok_or_else(|| PilError::OverflowError(format!("PA Paste {role} stride overflowed")))
}

fn invalid_row(role: &str) -> PilError {
    PilError::InternalError(format!(
        "PA NEON Paste {role} row was outside its validated storage"
    ))
}
