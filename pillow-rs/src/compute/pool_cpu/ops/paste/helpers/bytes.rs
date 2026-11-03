use std::ops::Deref;

use crate::error::PilError;
use crate::compute::pool_cpu::helpers::bytes::preblend;
use crate::raster::DynamicImage;

use super::layout::{MaskLayout, SourceLayout};
use super::plan::BytePlan;

/// Statically maps one accepted source layout into the destination pixel.
pub(crate) trait SourceMap<const SOURCE: usize, const DESTINATION: usize> {
    fn sample(source: &[u8], band: usize) -> u8;
}

pub(crate) struct Native;
pub(crate) struct PhysicalFourBytesToLumaAlpha;
pub(crate) struct PhysicalFourBytesToRgb;
pub(crate) struct PhysicalFourBytesToFour;

impl<const CHANNELS: usize> SourceMap<CHANNELS, CHANNELS> for Native {
    #[inline(always)]
    fn sample(source: &[u8], band: usize) -> u8 {
        source[band]
    }
}

impl SourceMap<4, 2> for PhysicalFourBytesToLumaAlpha {
    #[inline(always)]
    fn sample(source: &[u8], band: usize) -> u8 {
        source[if band == 0 { 0 } else { 3 }]
    }
}

impl SourceMap<4, 3> for PhysicalFourBytesToRgb {
    #[inline(always)]
    fn sample(source: &[u8], band: usize) -> u8 {
        source[band]
    }
}

impl SourceMap<4, 4> for PhysicalFourBytesToFour {
    #[inline(always)]
    fn sample(source: &[u8], band: usize) -> u8 {
        source[band]
    }
}

/// Executes the prepared, exact-mode byte strategy without mode conversion.
pub(crate) fn execute<const CHANNELS: usize, I>(
    plan: BytePlan<'_, I>,
) -> Result<Vec<u8>, PilError>
where
    I: Deref<Target = DynamicImage>,
{
    match plan.source_layout {
        SourceLayout::Native => execute_mapped::<CHANNELS, CHANNELS, Native, I>(plan),
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
    let mut output = plan.initialize_destination()?;
    if plan.region.is_empty() {
        return Ok(output);
    }
    if plan.mask.is_none() {
        copy_unmasked::<SOURCE, DESTINATION, I>(&plan, &mut output)?;
        return Ok(output);
    }
    let (source_width, _) = plan.source_size;
    let (destination_width, _) = plan.destination_size;
    let (region_width, _) = plan.region_size;
    let source_stride = checked_stride(source_width, SOURCE)?;
    let destination_stride = checked_stride(destination_width, DESTINATION)?;
    let source_row_width = usize::try_from(source_width)
        .map_err(|_| overflow("source width"))?;
    let destination_row_width = usize::try_from(destination_width)
        .map_err(|_| overflow("destination width"))?;
    let region_row_width = usize::try_from(region_width)
        .map_err(|_| overflow("region width"))?;
    let source_left = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?;
    let destination_left = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?;
    let copy_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?;
    let mask_layout = plan.mask_layout;
    let mask_stride = mask_layout
        .map(|layout| checked_stride(region_width, layout.channels))
        .transpose()?;
    let mask_bytes = plan.mask.as_deref().map(|image| image.as_bytes());

    for row in 0..usize::try_from(plan.region.height).map_err(|_| overflow("copy height"))? {
        let source_y = usize::try_from(plan.region.source_top)
            .map_err(|_| overflow("source y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("source row"))?;
        let destination_y = usize::try_from(plan.region.destination_top)
            .map_err(|_| overflow("destination y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("destination row"))?;
        let source_row = if plan.broadcast_source {
            &plan.source_bytes()[..SOURCE]
        } else {
            let start = source_y
                .checked_mul(source_stride)
                .ok_or_else(|| overflow("source row offset"))?;
            &plan.source_bytes()[start..start + source_stride]
        };
        let destination_start = destination_y
            .checked_mul(destination_stride)
            .ok_or_else(|| overflow("destination row offset"))?;
        let destination_row = &mut output[
            destination_start..destination_start + destination_row_width * DESTINATION
        ];
        let mask_row = match (mask_bytes, mask_stride) {
            (Some(bytes), Some(stride)) => {
                let start = source_y
                    .checked_mul(stride)
                    .ok_or_else(|| overflow("mask row offset"))?;
                Some(&bytes[start..start + stride])
            }
            _ => None,
        };
        apply_row::<SOURCE, DESTINATION, M>(
            source_row,
            destination_row,
            mask_row,
            mask_layout,
            source_left,
            destination_left,
            copy_width,
            source_row_width,
            region_row_width,
        );
    }
    Ok(output)
}

fn copy_unmasked<const SOURCE: usize, const DESTINATION: usize, I>(
    plan: &BytePlan<'_, I>,
    output: &mut [u8],
) -> Result<(), PilError>
where
    I: Deref<Target = DynamicImage>,
{
    match plan.source_layout {
        SourceLayout::Native => copy_native_rows::<SOURCE, DESTINATION, I>(plan, output),
        SourceLayout::PhysicalFourBytesToLumaAlpha => {
            copy_physical_four_bytes::<4, 2, PhysicalFourBytesToLumaAlpha, I>(plan, output)
        }
        SourceLayout::PhysicalFourBytesToRgb => {
            copy_physical_four_bytes::<4, 3, PhysicalFourBytesToRgb, I>(plan, output)
        }
        SourceLayout::PhysicalFourBytesToFour => {
            copy_physical_four_bytes::<4, 4, PhysicalFourBytesToFour, I>(plan, output)
        }
    }
}

fn copy_native_rows<const SOURCE: usize, const DESTINATION: usize, I>(
    plan: &BytePlan<'_, I>,
    output: &mut [u8],
) -> Result<(), PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let source_stride = checked_stride(plan.source_size.0, SOURCE)?;
    let destination_stride = checked_stride(plan.destination_size.0, DESTINATION)?;
    let source_left = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?;
    let destination_left = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?;
    let copy_width = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?;
    let copy_height = usize::try_from(plan.region.height)
        .map_err(|_| overflow("copy height"))?;
    let source_start_x = source_left
        .checked_mul(SOURCE)
        .ok_or_else(|| overflow("source row offset"))?;
    let destination_start_x = destination_left
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("destination row offset"))?;
    let copy_bytes = copy_width
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("row bytes"))?;

    for row in 0..copy_height {
        let source_y = usize::try_from(plan.region.source_top)
            .map_err(|_| overflow("source y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("source row"))?;
        let destination_y = usize::try_from(plan.region.destination_top)
            .map_err(|_| overflow("destination y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("destination row"))?;
        let source_start = source_y
            .checked_mul(source_stride)
            .and_then(|start| start.checked_add(source_start_x))
            .ok_or_else(|| overflow("source row offset"))?;
        let destination_start = destination_y
            .checked_mul(destination_stride)
            .and_then(|start| start.checked_add(destination_start_x))
            .ok_or_else(|| overflow("destination row offset"))?;
        output[destination_start..destination_start + copy_bytes]
            .copy_from_slice(&plan.source_bytes()[source_start..source_start + copy_bytes]);
    }
    Ok(())
}

fn copy_physical_four_bytes<const SOURCE: usize, const DESTINATION: usize, M, I>(
    plan: &BytePlan<'_, I>,
    output: &mut [u8],
) -> Result<(), PilError>
where
    M: SourceMap<SOURCE, DESTINATION>,
    I: Deref<Target = DynamicImage>,
{
    copy_mapped_rows::<SOURCE, DESTINATION, I>(plan, output, |source, destination| {
        for (source, destination) in source
            .chunks_exact(SOURCE)
            .zip(destination.chunks_exact_mut(DESTINATION))
        {
            for (band, output) in destination.iter_mut().enumerate() {
                *output = M::sample(source, band);
            }
        }
    })
}

fn copy_mapped_rows<const SOURCE: usize, const DESTINATION: usize, I>(
    plan: &BytePlan<'_, I>,
    output: &mut [u8],
    mut copy: impl FnMut(&[u8], &mut [u8]),
) -> Result<(), PilError>
where
    I: Deref<Target = DynamicImage>,
{
    let source_stride = checked_stride(plan.source_size.0, SOURCE)?;
    let destination_stride = checked_stride(plan.destination_size.0, DESTINATION)?;
    let source_start_x = usize::try_from(plan.region.source_left)
        .map_err(|_| overflow("source x"))?
        .checked_mul(SOURCE)
        .ok_or_else(|| overflow("source row offset"))?;
    let destination_start_x = usize::try_from(plan.region.destination_left)
        .map_err(|_| overflow("destination x"))?
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("destination row offset"))?;
    let copy_bytes = usize::try_from(plan.region.width)
        .map_err(|_| overflow("copy width"))?
        .checked_mul(DESTINATION)
        .ok_or_else(|| overflow("row bytes"))?;
    let copy_height = usize::try_from(plan.region.height)
        .map_err(|_| overflow("copy height"))?;

    for row in 0..copy_height {
        let source_y = usize::try_from(plan.region.source_top)
            .map_err(|_| overflow("source y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("source row"))?;
        let destination_y = usize::try_from(plan.region.destination_top)
            .map_err(|_| overflow("destination y"))?
            .checked_add(row)
            .ok_or_else(|| overflow("destination row"))?;
        let source_start = source_y
            .checked_mul(source_stride)
            .and_then(|start| start.checked_add(source_start_x))
            .ok_or_else(|| overflow("source row offset"))?;
        let destination_start = destination_y
            .checked_mul(destination_stride)
            .and_then(|start| start.checked_add(destination_start_x))
            .ok_or_else(|| overflow("destination row offset"))?;
        let source_width = usize::try_from(plan.region.width)
            .map_err(|_| overflow("copy width"))?
            .checked_mul(SOURCE)
            .ok_or_else(|| overflow("source row bytes"))?;
        copy(
            &plan.source_bytes()[source_start..source_start + source_width],
            &mut output[destination_start..destination_start + copy_bytes],
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_row<const SOURCE: usize, const DESTINATION: usize, M>(
    source_row: &[u8],
    destination_row: &mut [u8],
    mask_row: Option<&[u8]>,
    mask_layout: Option<MaskLayout>,
    source_left: usize,
    destination_left: usize,
    width: usize,
    source_row_width: usize,
    region_row_width: usize,
) where
    M: SourceMap<SOURCE, DESTINATION>,
{
    for column in 0..width {
        let source_x = source_left + column;
        let destination_x = destination_left + column;
        let source_pixel = if source_row_width == 1 {
            &source_row[..SOURCE]
        } else {
            let start = source_x * SOURCE;
            &source_row[start..start + SOURCE]
        };
        let destination_start = destination_x * DESTINATION;
        let destination_pixel =
            &mut destination_row[destination_start..destination_start + DESTINATION];
        let Some((mask_row, layout)) = mask_row.zip(mask_layout) else {
            copy_pixel::<SOURCE, DESTINATION, M>(source_pixel, destination_pixel);
            continue;
        };
        debug_assert!(source_x < region_row_width);
        let mask_start = source_x * layout.channels;
        let value = mask_row[mask_start + layout.value_index];
        if layout.binary && value == 0 {
            continue;
        }
        if value == 255 || layout.binary {
            copy_pixel::<SOURCE, DESTINATION, M>(source_pixel, destination_pixel);
            continue;
        }
        if value == 0 && !layout.premultiplied {
            continue;
        }
        blend_pixel::<SOURCE, DESTINATION, M>(source_pixel, destination_pixel, value, layout);
    }
}

#[inline(always)]
fn copy_pixel<const SOURCE: usize, const DESTINATION: usize, M>(
    source: &[u8],
    destination: &mut [u8],
) where
    M: SourceMap<SOURCE, DESTINATION>,
{
    for (band, output) in destination.iter_mut().enumerate() {
        *output = M::sample(source, band);
    }
}

#[inline(always)]
fn blend_pixel<const SOURCE: usize, const DESTINATION: usize, M>(
    source: &[u8],
    destination: &mut [u8],
    mask: u8,
    layout: MaskLayout,
) where
    M: SourceMap<SOURCE, DESTINATION>,
{
    let inverse = 255 - u16::from(mask);
    let weight = u16::from(mask);
    for (band, output) in destination.iter_mut().enumerate() {
        let source = M::sample(source, band);
        *output = if layout.premultiplied {
            preblend::apply(source, *output, mask)
        } else {
            ((u16::from(source) * weight + u16::from(*output) * inverse + 127) / 255) as u8
        };
    }
}

fn checked_stride(width: u32, channels: usize) -> Result<usize, PilError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(channels))
        .ok_or_else(|| overflow("row stride"))
}

fn overflow(part: &str) -> PilError {
    PilError::OverflowError(format!("Paste {part} overflowed"))
}
