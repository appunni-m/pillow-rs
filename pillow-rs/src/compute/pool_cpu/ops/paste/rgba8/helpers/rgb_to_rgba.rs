//! Unmasked native RGB to RGBA region copy.

use crate::compute::pool_cpu::helpers::channel::rgba::rgb_to_rgba_block;
use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::SourceLayout,
    plan::BytePlan,
    region::checked_row_range,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn supports(plan: &BytePlan<'_>) -> bool {
    plan.source.mode() == Mode::ImageRgb8
        && plan.source_layout == SourceLayout::RgbToRgba
        && !plan.broadcast_source
        && plan.mask.is_none()
}

pub(crate) fn execute(plan: &BytePlan<'_>) -> Result<Vec<u8>, PilError> {
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        crate::compute::record_pipeline_operation_path("vector-rgb-to-rgba-copy");
        return Ok(output);
    }
    let source_stride = checked_stride(plan.source_size.0, 3)?;
    let destination_stride = checked_stride(plan.destination_size.0, 4)?;
    let source = plan.source.as_bytes();

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
            3,
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
        let source_row = source
            .get(source_range)
            .ok_or_else(|| invalid_shape("source"))?;
        let destination_row = output
            .get_mut(destination_range)
            .ok_or_else(|| invalid_shape("destination"))?;
        copy_row(source_row, destination_row);
    }

    let width = u64::from(plan.region.width);
    let height = u64::from(plan.region.height);
    let vector_blocks = (width / 4).saturating_mul(height);
    let scalar_tail = (width % 4).saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("vector-rgb-to-rgba-copy");
    Ok(output)
}

fn copy_row(source: &[u8], destination: &mut [u8]) {
    debug_assert_eq!(source.len() % 3, 0);
    debug_assert_eq!(destination.len(), source.len() / 3 * 4);
    let mut source_blocks = source.chunks_exact(12);
    let mut destination_blocks = destination.chunks_exact_mut(16);
    for (source_block, destination_block) in source_blocks
        .by_ref()
        .zip(destination_blocks.by_ref())
    {
        destination_block.copy_from_slice(&rgb_to_rgba_block(source_block, 255));
    }

    let source_tail = source_blocks.remainder();
    let destination_tail = destination_blocks.into_remainder();
    for (rgb, rgba) in source_tail
        .chunks_exact(3)
        .zip(destination_tail.chunks_exact_mut(4))
    {
        rgba[..3].copy_from_slice(rgb);
        rgba[3] = 255;
    }
}

fn checked_stride(width: u32, channels: usize) -> Result<usize, PilError> {
    usize::try_from(width)
        .map_err(|_| overflow("image width"))?
        .checked_mul(channels)
        .ok_or_else(|| overflow("row stride"))
}

fn invalid_shape(part: &'static str) -> PilError {
    PilError::InternalError(format!("RGB-to-RGBA Paste {part} row did not match its plan"))
}

fn overflow(part: &'static str) -> PilError {
    PilError::OverflowError(format!("RGB-to-RGBA Paste {part} overflowed"))
}
