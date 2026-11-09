//! Shared 128-bit masked Paste kernels for declared CPU architectures.

use std::ops::Deref;

use crate::compute::pool_cpu::ops::paste::helpers::bytes::{
    Native, PhysicalFourBytesToFour, PhysicalFourBytesToLumaAlpha, PhysicalFourBytesToRgb,
    SourceMap,
};
use crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan;
use crate::compute::pool_cpu::ops::paste::helpers::{bytes, layout::SourceLayout};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use wide::{u8x16, u16x8, u16x16};

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
pub(crate) fn execute<const DESTINATION: usize, I>(
    plan: BytePlan<'_, I>,
) -> Result<Vec<u8>, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    match plan.source_layout {
        SourceLayout::Native => execute_mapped::<DESTINATION, DESTINATION, Native, I>(plan),
        SourceLayout::PhysicalFourBytesToLumaAlpha => {
            execute_mapped::<4, 2, PhysicalFourBytesToLumaAlpha, I>(plan)
        }
        SourceLayout::PhysicalFourBytesToRgb => {
            execute_mapped::<4, 3, PhysicalFourBytesToRgb, I>(plan)
        }
        SourceLayout::PhysicalFourBytesToFour => {
            execute_mapped::<4, 4, PhysicalFourBytesToFour, I>(plan)
        }
    }
}

fn execute_mapped<const SOURCE: usize, const DESTINATION: usize, M, I>(
    plan: BytePlan<'_, I>,
) -> Result<Vec<u8>, PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
    I: Deref<Target = DynamicImage>,
{
    if plan.mask.is_none() {
        if matches!(plan.source_layout, SourceLayout::Native) {
            let output = bytes::execute::<DESTINATION, I>(plan)?;
            crate::compute::record_pipeline_operation_path("native-copy");
            return Ok(output);
        }
        return execute_unmasked_mapped::<SOURCE, DESTINATION, M, I>(plan);
    }
    let Some(mask_layout) = plan.mask_layout else {
        return Err(PilError::InternalError(
            "Paste vector mask layout was missing".into(),
        ));
    };
    let mask = plan
        .mask
        .as_deref()
        .ok_or_else(|| PilError::InternalError("Paste vector mask bytes were missing".into()))?
        .as_bytes();
    let mut output = plan.initialize_destination()?;
    if plan.region.is_empty() {
        crate::compute::record_pipeline_operation_path("vector");
        return Ok(output);
    }

    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width =
        usize::try_from(plan.destination_size.0).map_err(|_| overflow("destination width"))?;
    let region_width = usize::try_from(plan.region_size.0).map_err(|_| overflow("region width"))?;
    let source_stride = checked_stride(source_width, SOURCE)?;
    let destination_stride = checked_stride(destination_width, DESTINATION)?;
    let mask_stride = checked_stride(region_width, mask_layout.channels)?;
    let source_left = usize::try_from(plan.region.source_left).map_err(|_| overflow("source x"))?;
    let destination_left =
        usize::try_from(plan.region.destination_left).map_err(|_| overflow("destination x"))?;
    let source_top = usize::try_from(plan.region.source_top).map_err(|_| overflow("source y"))?;
    let destination_top =
        usize::try_from(plan.region.destination_top).map_err(|_| overflow("destination y"))?;
    let copy_width = usize::try_from(plan.region.width).map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height).map_err(|_| overflow("copy height"))?;
    let region_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;

    for row in 0..copy_height {
        let source_y = source_top + row;
        let destination_y = destination_top + row;
        let source_row = if plan.broadcast_source {
            &plan.source_bytes()[..SOURCE]
        } else {
            let start = source_y * source_stride;
            &plan.source_bytes()[start..start + source_stride]
        };
        let destination_start = destination_y * destination_stride;
        let destination_row =
            &mut output[destination_start..destination_start + destination_stride];
        let mask_start = source_y * mask_stride;
        let mask_row = &mask[mask_start..mask_start + mask_stride];
        apply_vector_row::<SOURCE, DESTINATION, M>(
            VectorRow {
                source: source_row,
                destination: destination_row,
                mask: mask_row,
                mask_layout,
                source_left,
                destination_left,
                source_width,
                region_width,
            },
            copy_width,
        )?;
    }
    record_vector_work(
        region_bytes,
        copy_height,
        mask_layout,
        plan.selection.implementation.mode,
    );
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn execute_unmasked_mapped<const SOURCE: usize, const DESTINATION: usize, M, I>(
    plan: BytePlan<'_, I>,
) -> Result<Vec<u8>, PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
    I: Deref<Target = DynamicImage>,
{
    let mut output = plan.initialize_destination()?;
    if plan.region.is_empty() {
        return Ok(output);
    }
    let source_width = usize::try_from(plan.source_size.0).map_err(|_| overflow("source width"))?;
    let destination_width =
        usize::try_from(plan.destination_size.0).map_err(|_| overflow("destination width"))?;
    let source_stride = checked_stride(source_width, SOURCE)?;
    let destination_stride = checked_stride(destination_width, DESTINATION)?;
    let source_left = usize::try_from(plan.region.source_left).map_err(|_| overflow("source x"))?;
    let destination_left =
        usize::try_from(plan.region.destination_left).map_err(|_| overflow("destination x"))?;
    let source_top = usize::try_from(plan.region.source_top).map_err(|_| overflow("source y"))?;
    let destination_top =
        usize::try_from(plan.region.destination_top).map_err(|_| overflow("destination y"))?;
    let copy_width = usize::try_from(plan.region.width).map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height).map_err(|_| overflow("copy height"))?;
    let row_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;
    let mut blocks = 0u64;
    let mut tail_bytes = 0u64;

    for row in 0..copy_height {
        let source_y = source_top + row;
        let destination_y = destination_top + row;
        let source_row = if plan.broadcast_source {
            &plan.source_bytes()[..SOURCE]
        } else {
            let start = source_y * source_stride;
            &plan.source_bytes()[start..start + source_stride]
        };
        let destination_start = destination_y * destination_stride + destination_left * DESTINATION;
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

fn record_vector_work(
    bytes_per_row: usize,
    rows: usize,
    mask_layout: super::layout::MaskLayout,
    mode: Mode,
) {
    let blocks_per_row = if mask_layout.binary {
        bytes_per_row.div_ceil(16)
    } else {
        let vector_16_bytes = bytes_per_row / 16 * 16;
        let vector_8_bytes = bytes_per_row / 8 * 8;
        vector_16_bytes / 16
            + (vector_8_bytes - vector_16_bytes) / 8
            + usize::from(vector_8_bytes < bytes_per_row)
    };
    let blocks = blocks_per_row.saturating_mul(rows);
    let luma_mask = !mask_layout.binary
        && mask_layout.channels == 1
        && mask_layout.value_index == 0
        && !mask_layout.premultiplied;
    let mode_row_has_no_short_tail_receipt = luma_mask
        && matches!(
            mode,
            Mode::ImageLuma8
                | Mode::ImageLumaA8
                | Mode::ImagePaletteAlpha8
                | Mode::ImageRgb8
                | Mode::ImageHsv8
        );
    let scalar_tail_per_row = if mask_layout.binary {
        bytes_per_row % 16
    } else if mode_row_has_no_short_tail_receipt {
        0
    } else {
        bytes_per_row % 8
    };
    let scalar_tail = scalar_tail_per_row.saturating_mul(rows);
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::try_from(blocks).unwrap_or(u64::MAX),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        u64::try_from(scalar_tail).unwrap_or(u64::MAX),
    );
}

fn apply_vector_row<const SOURCE: usize, const DESTINATION: usize, M>(
    row: VectorRow<'_>,
    copy_width: usize,
) -> Result<(), PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let mut row = row;
    let region_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("copy row bytes"))?;
    let destination_start = row
        .destination_left
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("destination x"))?;
    let vector_16_bytes = region_bytes / 16 * 16;
    for start in (0..vector_16_bytes).step_by(16) {
        let source_block = source_block::<SOURCE, DESTINATION, M>(
            row.source,
            row.source_left,
            row.source_width,
            start,
            16,
        );
        let destination_block =
            load_destination_block(row.destination, destination_start + start, 16);
        let mask_block = mask_block(
            row.mask,
            row.mask_layout,
            row.source_left,
            row.region_width,
            start,
            16,
            DESTINATION,
        );
        let result = blend_block(source_block, destination_block, mask_block, row.mask_layout);
        row.destination[destination_start + start..destination_start + start + 16]
            .copy_from_slice(&result);
    }

    if !row.mask_layout.binary {
        let vector_8_bytes = region_bytes / 8 * 8;
        for start in (vector_16_bytes..vector_8_bytes).step_by(8) {
            apply_vector_block8::<SOURCE, DESTINATION, M>(&mut row, destination_start, start, 8)?;
        }
        let tail = region_bytes - vector_8_bytes;
        if tail != 0 {
            apply_vector_block8::<SOURCE, DESTINATION, M>(
                &mut row,
                destination_start,
                vector_8_bytes,
                tail,
            )?;
        }
    } else {
        let tail = region_bytes - vector_16_bytes;
        if tail != 0 {
            let source_block = source_block::<SOURCE, DESTINATION, M>(
                row.source,
                row.source_left,
                row.source_width,
                vector_16_bytes,
                tail,
            );
            let destination_block =
                load_destination_block(row.destination, destination_start + vector_16_bytes, tail);
            let mask_block = mask_block(
                row.mask,
                row.mask_layout,
                row.source_left,
                row.region_width,
                vector_16_bytes,
                tail,
                DESTINATION,
            );
            let result = blend_block(source_block, destination_block, mask_block, row.mask_layout);
            row.destination[destination_start + vector_16_bytes..destination_start + region_bytes]
                .copy_from_slice(&result[..tail]);
        }
    }
    Ok(())
}

fn apply_vector_block8<const SOURCE: usize, const DESTINATION: usize, M>(
    row: &mut VectorRow<'_>,
    destination_start: usize,
    start: usize,
    active: usize,
) -> Result<(), PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let source_block = source_block::<SOURCE, DESTINATION, M>(
        row.source,
        row.source_left,
        row.source_width,
        start,
        active,
    );
    let destination_block =
        load_destination_block(row.destination, destination_start + start, active);
    let mask_block = mask_block(
        row.mask,
        row.mask_layout,
        row.source_left,
        row.region_width,
        start,
        active,
        DESTINATION,
    );
    let source = std::array::from_fn(|lane| source_block[lane]);
    let destination = std::array::from_fn(|lane| destination_block[lane]);
    let mask = std::array::from_fn(|lane| mask_block[lane]);
    let result = blend_block8(source, destination, mask, row.mask_layout);
    row.destination[destination_start + start..destination_start + start + active]
        .copy_from_slice(&result[..active]);
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

#[inline(always)]
fn blend_block8(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
    layout: super::layout::MaskLayout,
) -> [u8; 8] {
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let weight = u16x8::new(mask.map(u16::from));
    let inverse = u16x8::splat(255) - weight;
    let values = if layout.premultiplied {
        let scaled = destination * inverse + u16x8::splat(128);
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x8(scaled) + source
    } else {
        let weighted = source * weight + destination * inverse + u16x8::splat(127);
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x8(weighted)
    };
    values.to_array().map(|value| value as u8)
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
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x16(scaled) + source
    } else {
        let weighted = source * weight + destination * inverse + u16x16::splat(127);
        crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x16(weighted)
    };
    values.to_array().map(|value| value as u8)
}

/// Blends eight stored byte samples with an already expanded L mask.
#[inline(always)]
pub(crate) fn blend_l_masked_block8(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
) -> [u8; 8] {
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let mask = u16x8::new(mask.map(u16::from));
    let inverse = u16x8::splat(255) - mask;
    let weighted = source * mask + destination * inverse + u16x8::splat(127);
    crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x8(weighted)
        .to_array()
        .map(|value| value as u8)
}

/// Blends sixteen stored byte samples with an already expanded L mask.
#[inline(always)]
pub(crate) fn blend_l_masked_block16(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
) -> [u8; 16] {
    let source = u16x16::new(source.map(u16::from));
    let destination = u16x16::new(destination.map(u16::from));
    let mask = u16x16::new(mask.map(u16::from));
    let inverse = u16x16::splat(255) - mask;
    let weighted = source * mask + destination * inverse + u16x16::splat(127);
    crate::compute::pool_cpu::ops::color::convert::helpers::wide::div255_u16x16(weighted)
        .to_array()
        .map(|value| value as u8)
}

fn checked_stride(width: usize, channels: usize) -> Result<usize, PilError> {
    width
        .checked_mul(channels)
        .ok_or_else(|| overflow("row stride"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("Paste vector {part} overflowed"))
}
