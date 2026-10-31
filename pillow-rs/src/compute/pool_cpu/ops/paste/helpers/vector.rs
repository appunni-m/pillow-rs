//! Shared 128-bit masked Paste kernels for declared CPU architectures.
//!
//! TODO(pillow-rs#3ptb): restore the archived architecture-tuned NEON mask
//! blends for L/LA/RGB/RGBA/CMYK and dedicated RGB-to-RGBA/RGBA-to-RGB
//! shuffles. Keep these shared kernels as the exact fallback until those
//! specialized paths are migrated and measured.

use crate::compute::pool_cpu::ops::paste::helpers::bytes::{
    FourBytesToRgb, LumaAlphaToRgb, Native, RgbToRgba, SourceMap,
};
use crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan;
use crate::compute::pool_cpu::ops::paste::helpers::{bytes, layout::SourceLayout};
use crate::error::PilError;
use wide::{u8x16, u16x16};

struct VectorRow<'a> {
    source: &'a [u8],
    destination: &'a mut [u8],
    mask: &'a [u8],
    mask_layout: super::layout::MaskLayout,
    source_left: usize,
    destination_left: usize,
    source_width: usize,
    region_width: usize,
}

/// Runs the native 128-bit masked kernel selected by an exact mode strategy.
pub(crate) fn execute<const DESTINATION: usize>(
    plan: BytePlan<'_>,
) -> Result<Vec<u8>, PilError> {
    match plan.source_layout {
        SourceLayout::Native => execute_mapped::<DESTINATION, DESTINATION, Native>(plan),
        SourceLayout::LumaAlphaToRgb => execute_mapped::<2, 3, LumaAlphaToRgb>(plan),
        SourceLayout::FourBytesToRgb => execute_mapped::<4, 3, FourBytesToRgb>(plan),
        SourceLayout::RgbToRgba => execute_mapped::<3, 4, RgbToRgba>(plan),
    }
}

fn execute_mapped<const SOURCE: usize, const DESTINATION: usize, M>(
    plan: BytePlan<'_>,
) -> Result<Vec<u8>, PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    if plan.mask.is_none() {
        if matches!(plan.source_layout, SourceLayout::Native) {
            let output = bytes::execute::<DESTINATION>(plan)?;
            crate::compute::record_pipeline_operation_path("native-copy");
            return Ok(output);
        }
        return execute_unmasked_mapped::<SOURCE, DESTINATION, M>(plan);
    }
    let Some(mask_layout) = plan.mask_layout else {
        return Err(PilError::InternalError("Paste vector mask layout was missing".into()));
    };
    let mask = plan
        .mask
        .as_deref()
        .ok_or_else(|| PilError::InternalError("Paste vector mask bytes were missing".into()))?
        .as_bytes();
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        return Ok(output);
    }

    let source_width = usize::try_from(plan.source_size.0)
        .map_err(|_| overflow("source width"))?;
    let destination_width = usize::try_from(plan.destination_size.0)
        .map_err(|_| overflow("destination width"))?;
    let region_width = usize::try_from(plan.region_size.0)
        .map_err(|_| overflow("region width"))?;
    let source_stride = checked_stride(source_width, SOURCE)?;
    let destination_stride = checked_stride(destination_width, DESTINATION)?;
    let mask_stride = checked_stride(region_width, mask_layout.channels)?;
    let source_left = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?;
    let destination_left = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?;
    let source_top = usize::try_from(plan.region.source_top)
        .map_err(|_| overflow("source y"))?;
    let destination_top = usize::try_from(plan.region.destination_top)
        .map_err(|_| overflow("destination y"))?;
    let copy_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height)
        .map_err(|_| overflow("copy height"))?;
    let region_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;

    for row in 0..copy_height {
        let source_y = source_top + row;
        let destination_y = destination_top + row;
        let source_row = if plan.broadcast_source {
            &plan.source.as_bytes()[..SOURCE]
        } else {
            let start = source_y * source_stride;
            &plan.source.as_bytes()[start..start + source_stride]
        };
        let destination_start = destination_y * destination_stride;
        let destination_row = &mut output[destination_start..destination_start + destination_stride];
        let mask_start = source_y * mask_stride;
        let mask_row = &mask[mask_start..mask_start + mask_stride];
        apply_vector_row::<SOURCE, DESTINATION, M>(VectorRow {
            source: source_row,
            destination: destination_row,
            mask: mask_row,
            mask_layout,
            source_left,
            destination_left,
            source_width,
            region_width,
        }, copy_width)?;
    }
    record_vector_work(region_bytes, copy_height);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn execute_unmasked_mapped<const SOURCE: usize, const DESTINATION: usize, M>(
    plan: BytePlan<'_>,
) -> Result<Vec<u8>, PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let mut output = plan.destination.to_vec();
    if plan.region.is_empty() {
        return Ok(output);
    }
    let source_width = usize::try_from(plan.source_size.0)
        .map_err(|_| overflow("source width"))?;
    let destination_width = usize::try_from(plan.destination_size.0)
        .map_err(|_| overflow("destination width"))?;
    let source_stride = checked_stride(source_width, SOURCE)?;
    let destination_stride = checked_stride(destination_width, DESTINATION)?;
    let source_left = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?;
    let destination_left = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?;
    let source_top = usize::try_from(plan.region.source_top)
        .map_err(|_| overflow("source y"))?;
    let destination_top = usize::try_from(plan.region.destination_top)
        .map_err(|_| overflow("destination y"))?;
    let copy_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height)
        .map_err(|_| overflow("copy height"))?;
    let row_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;
    let mut blocks = 0u64;
    let mut tail_bytes = 0u64;

    for row in 0..copy_height {
        let source_y = source_top + row;
        let destination_y = destination_top + row;
        let source_row = if plan.broadcast_source {
            &plan.source.as_bytes()[..SOURCE]
        } else {
            let start = source_y * source_stride;
            &plan.source.as_bytes()[start..start + source_stride]
        };
        let destination_start = destination_y * destination_stride
            + destination_left * DESTINATION;
        let destination_row = &mut output[destination_start..destination_start + row_bytes];
        for start in (0..row_bytes).step_by(16) {
            let active = (row_bytes - start).min(16);
            let block = source_block::<SOURCE, DESTINATION, M>(
                source_row,
                source_left,
                source_width,
                start,
                active,
            );
            destination_row[start..start + active].copy_from_slice(&block[..active]);
            blocks = blocks.saturating_add(1);
            if active != 16 {
                tail_bytes = tail_bytes.saturating_add(active as u64);
            }
        }
    }
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(tail_bytes);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn record_vector_work(bytes_per_row: usize, rows: usize) {
    let blocks = bytes_per_row.div_ceil(16).saturating_mul(rows);
    let tail = (bytes_per_row % 16).saturating_mul(rows);
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::try_from(blocks).unwrap_or(u64::MAX),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        u64::try_from(tail).unwrap_or(u64::MAX),
    );
}

fn apply_vector_row<const SOURCE: usize, const DESTINATION: usize, M>(
    row: VectorRow<'_>,
    copy_width: usize,
) -> Result<(), PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let VectorRow {
        source,
        destination,
        mask,
        mask_layout,
        source_left,
        destination_left,
        source_width,
        region_width,
    } = row;
    let region_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;
    let destination_start = destination_left
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("destination x"))?;
    for start in (0..region_bytes).step_by(16) {
        let active = (region_bytes - start).min(16);
        let source_block = source_block::<SOURCE, DESTINATION, M>(
            source,
            source_left,
            source_width,
            start,
            active,
        );
        let destination_block = load_destination_block(
            destination,
            destination_start + start,
            active,
        );
        let mask_block = mask_block(
            mask,
            mask_layout,
            source_left,
            region_width,
            start,
            active,
            DESTINATION,
        );
        let result = blend_block(source_block, destination_block, mask_block, mask_layout);
        destination[destination_start + start..destination_start + start + active]
            .copy_from_slice(&result[..active]);
    }
    Ok(())
}

fn source_block<const SOURCE: usize, const DESTINATION: usize, M>(
    source: &[u8],
    source_left: usize,
    source_width: usize,
    start: usize,
    active: usize,
) -> [u8; 16]
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let mut block = [0u8; 16];
    for (lane, output) in block.iter_mut().enumerate().take(active) {
        let byte = start + lane;
        let source_pixel = source_left + byte / DESTINATION;
        let source_band = byte % DESTINATION;
        let input = if source_width == 1 {
            &source[..SOURCE]
        } else {
            let pixel_start = source_pixel * SOURCE;
            &source[pixel_start..pixel_start + SOURCE]
        };
        *output = M::sample(input, source_band);
    }
    block
}

fn load_destination_block(destination: &[u8], start: usize, active: usize) -> [u8; 16] {
    let mut block = [0u8; 16];
    block[..active].copy_from_slice(&destination[start..start + active]);
    block
}

fn mask_block(
    mask: &[u8],
    layout: super::layout::MaskLayout,
    source_left: usize,
    region_width: usize,
    start: usize,
    active: usize,
    destination_channels: usize,
) -> [u8; 16] {
    let mut block = [0u8; 16];
    for (lane, output) in block.iter_mut().enumerate().take(active) {
        let pixel = start + lane;
        let mask_pixel = source_left + pixel / destination_channels;
        debug_assert!(mask_pixel < region_width);
        *output = mask[mask_pixel * layout.channels + layout.value_index];
    }
    block
}

#[inline(always)]
fn blend_block(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
    layout: super::layout::MaskLayout,
) -> [u8; 16] {
    if layout.binary {
        let selected = u8x16::new(mask)
            .simd_eq(u8x16::splat(0))
            .bitselect(u8x16::new(destination), u8x16::new(source));
        return selected.to_array();
    }
    let source = u16x16::new(source.map(u16::from));
    let destination = u16x16::new(destination.map(u16::from));
    let weight = u16x16::new(mask.map(u16::from));
    let inverse = u16x16::splat(255) - weight;
    let values = if layout.premultiplied {
        let scaled = destination * inverse + u16x16::splat(128);
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x16(scaled)
            + source
    } else {
        let weighted = source * weight + destination * inverse + u16x16::splat(127);
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x16(weighted)
    };
    values.to_array().map(|value| value as u8)
}

fn checked_stride(width: usize, channels: usize) -> Result<usize, PilError> {
    width.checked_mul(channels).ok_or_else(|| overflow("row stride"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("Paste vector {part} overflowed"))
}
