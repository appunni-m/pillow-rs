//! Unmasked native RGBA to RGB region copy.

use crate::compute::pool_cpu::helpers::channel::rgba::rgba_to_rgb_block;
use crate::compute::pool_cpu::ops::paste::helpers::{
    layout::SourceLayout,
    plan::BytePlan,
    region::checked_row_range,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn supports<I>(plan: &BytePlan<'_, I>) -> bool
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    plan.source.mode() == Mode::ImageRgba8
        && plan.source_layout == SourceLayout::PhysicalFourBytesToRgb
        && !plan.broadcast_source
        && plan.mask.is_none()
}

pub(crate) fn execute<I>(plan: &BytePlan<'_, I>) -> Result<Vec<u8>, PilError>
where
    I: std::ops::Deref<Target = crate::raster::DynamicImage>,
{
    let mut output = plan.initialize_destination()?;
    if plan.region.is_empty() {
        crate::compute::record_pipeline_operation_path("vector");
        return Ok(output);
    }
    let source_stride = checked_stride(plan.source_size.0, 4)?;
    let destination_stride = checked_stride(plan.destination_size.0, 3)?;
    let source = plan.source_bytes();

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
            3,
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

    let vector_blocks = u64::from(plan.region.width)
        .div_ceil(4)
        .saturating_mul(u64::from(plan.region.height));
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn copy_row(source: &[u8], destination: &mut [u8]) {
    debug_assert_eq!(source.len() % 4, 0);
    debug_assert_eq!(destination.len(), source.len() / 4 * 3);
    for (source_block, destination_block) in source.chunks(16).zip(destination.chunks_mut(12)) {
        let rgb_block = rgba_to_rgb_block(source_block);
        destination_block.copy_from_slice(&rgb_block[..destination_block.len()]);
    }
}

fn checked_stride(width: u32, channels: usize) -> Result<usize, PilError> {
    usize::try_from(width)
        .map_err(|_| overflow("image width"))?
        .checked_mul(channels)
        .ok_or_else(|| overflow("row stride"))
}

fn invalid_shape(part: &'static str) -> PilError {
    PilError::InternalError(format!("RGBA-to-RGB Paste {part} row did not match its plan"))
}

fn overflow(part: &'static str) -> PilError {
    PilError::OverflowError(format!("RGBA-to-RGB Paste {part} overflowed"))
}
