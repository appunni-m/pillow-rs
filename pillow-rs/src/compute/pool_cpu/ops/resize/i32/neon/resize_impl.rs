//! Filtered ImageI32 Resize with Pillow's two-pass rounding boundary.

use crate::checked_dims::CheckedDims;
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::compute::helpers::resampling::rounding::round_up;
use crate::ops::pil_resize::{FilterCoeffsF64, i32_samples_from_le_bytes};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;
use wide::f64x8;

use super::super::super::helpers::strategy::require_architecture;
use super::super::super::helpers::typed_nearest::BoxBounds;
use super::kernels;
use super::plan::{self, HorizontalPlan};
use super::LANES;

pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
    architecture: Architecture,
) -> Result<RgbaImage, PilError> {
    require_architecture(context, architecture)?;
    if architecture != Architecture::Neon || bounds.is_some() {
        return Err(PilError::InternalError(
            "ImageI32 NEON Resize received an unsupported request".into(),
        ));
    }
    let (source_width, source_height) = input.dimensions();
    let source_dims = CheckedDims::new(source_width, source_height, 4)?;
    if input.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "ImageI32 NEON Resize source buffer shape mismatch".into(),
        ));
    }
    if matches!(filter, ResampleFilter::Nearest) {
        return resize_nearest(input, width, height, source_dims.total_pixels());
    }
    let output_dims = CheckedDims::new(width, height, 4)?;
    let source = i32_samples_from_le_bytes(input.as_raw(), source_dims.total_pixels());
    let (horizontal, vertical) = super::super::super::helpers::coefficients::for_resize(
        (source_width, source_height),
        (width, height),
        filter,
        None,
    );
    let horizontal_plan = plan::build_horizontal(&horizontal, source_width as usize, width as usize)
        .ok_or_else(invalid_plan)?;
    let (output, blocks, used_neon) = execute_filtered(
        &source,
        source_width as usize,
        source_height as usize,
        width as usize,
        height as usize,
        &horizontal_plan,
        &vertical,
        output_dims,
    )?;
    record_path(blocks, used_neon);
    RgbaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}

fn resize_nearest(
    input: &RgbaImage,
    width: u32,
    height: u32,
    source_pixels: usize,
) -> Result<RgbaImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    let source = i32_samples_from_le_bytes(input.as_raw(), source_pixels);
    let x_indices = nearest_indices(source_width, width).ok_or_else(invalid_plan)?;
    let y_indices = nearest_indices(source_height, height).ok_or_else(invalid_plan)?;
    let output_dims = CheckedDims::new(width, height, 4)?;
    let mut output = output_dims.alloc_buffer();
    let mut vector_blocks = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y.checked_mul(source_width as usize).ok_or_else(invalid_plan)?;
        let output_row = output_y
            .checked_mul(width as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(invalid_plan)?;
        for output_x in (0..width as usize).step_by(LANES) {
            let count = (width as usize - output_x).min(LANES);
            let values = std::array::from_fn(|lane| {
                if lane < count {
                    source[source_row + x_indices[output_x + lane]]
                } else {
                    0
                }
            });
            let packed = wide::i32x8::new(values).to_array();
            for lane in 0..count {
                let start = output_row
                    .checked_add((output_x + lane).checked_mul(4).ok_or_else(invalid_plan)?)
                    .ok_or_else(invalid_plan)?;
                output
                    .get_mut(start..start.checked_add(4).ok_or_else(invalid_plan)?)
                    .ok_or_else(invalid_plan)?
                    .copy_from_slice(&packed[lane].to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    RgbaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}

fn nearest_indices(source_size: u32, output_size: u32) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 {
        return None;
    }
    let scale = f64::from(source_size) / f64::from(output_size);
    let mut position = scale * 0.5;
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    for _ in 0..output_size {
        let index = position as u32;
        indices.push(if index >= source_size {
            usize::try_from(source_size - 1).ok()?
        } else {
            usize::try_from(index).ok()?
        });
        position += scale;
    }
    Some(indices)
}

fn execute_filtered(
    source: &[i32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    horizontal: &HorizontalPlan,
    vertical: &FilterCoeffsF64,
    output_dims: CheckedDims,
) -> Result<(Vec<u8>, u64, bool), PilError> {
    let intermediate_len = source_height
        .checked_mul(output_width)
        .ok_or_else(invalid_plan)?;
    let mut intermediate = vec![0i32; intermediate_len];
    let (horizontal_blocks, horizontal_neon) = horizontal_pass(
        source,
        source_width,
        source_height,
        output_width,
        horizontal,
        &mut intermediate,
    )?;
    let mut output = output_dims.alloc_buffer();
    let (vertical_blocks, vertical_neon) = vertical_pass(
        &intermediate,
        output_width,
        output_height,
        vertical,
        &mut output,
    )?;
    Ok((
        output,
        horizontal_blocks.saturating_add(vertical_blocks),
        horizontal_neon || vertical_neon,
    ))
}

fn horizontal_pass(
    source: &[i32],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    plan: &HorizontalPlan,
    intermediate: &mut [i32],
) -> Result<(u64, bool), PilError> {
    let mut blocks = 0u64;
    let mut used_neon = false;
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_width).ok_or_else(invalid_plan)?;
        let source_row = source
            .get(source_start..source_start.checked_add(source_width).ok_or_else(invalid_plan)?)
            .ok_or_else(invalid_plan)?;
        let intermediate_row = source_y.checked_mul(output_width).ok_or_else(invalid_plan)?;
        for (block_index, block) in plan.blocks.iter().enumerate() {
            let output_x = block_index.checked_mul(LANES).ok_or_else(invalid_plan)?;
            let count = (output_width - output_x).min(LANES);
            let destination_start = intermediate_row.checked_add(output_x).ok_or_else(invalid_plan)?;
            let destination = intermediate
                .get_mut(destination_start..destination_start.checked_add(count).ok_or_else(invalid_plan)?)
                .ok_or_else(invalid_plan)?;
            if let Some(values) = kernels::horizontal_block(source_row, block) {
                destination.copy_from_slice(&values[..count]);
                used_neon = true;
            } else {
                horizontal_wide_block(source_row, block, destination);
            }
            blocks = blocks.saturating_add(1);
        }
    }
    Ok((blocks, used_neon))
}

fn horizontal_wide_block(
    source_row: &[i32],
    block: &plan::HorizontalBlock,
    destination: &mut [i32],
) {
    let mut sums = f64x8::splat(0.0);
    for tap in &block.taps {
        sums = tap
            .weights
            .mul_add(f64x8::new(plan::horizontal_values(source_row, tap)), sums);
    }
    for (slot, value) in destination.iter_mut().zip(sums.to_array()) {
        *slot = round_up(value) as i32;
    }
}

fn vertical_pass(
    intermediate: &[i32],
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffsF64,
    output: &mut [u8],
) -> Result<(u64, bool), PilError> {
    let mut blocks = 0u64;
    let mut used_neon = false;
    for output_y in 0..output_height {
        let first_source_y = usize::try_from(
            *coefficients.xmin.get(output_y).ok_or_else(invalid_plan)?,
        )
        .map_err(|_| invalid_plan())?;
        let weights = coefficients.weights.get(output_y).ok_or_else(invalid_plan)?;
        let output_row = output_y
            .checked_mul(output_width)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(invalid_plan)?;
        for output_x in (0..output_width).step_by(LANES) {
            let count = (output_width - output_x).min(LANES);
            let values = if count == LANES {
                kernels::vertical_block(
                    intermediate,
                    output_width,
                    output_x,
                    first_source_y,
                    weights,
                )
            } else {
                None
            };
            let rounded = match values {
                Some(values) => {
                    used_neon = true;
                    values
                }
                None => vertical_wide_block(
                    intermediate,
                    output_width,
                    output_x,
                    count,
                    first_source_y,
                    weights,
                )?,
            };
            write_output_block(output, output_row, output_x, count, &rounded)?;
            blocks = blocks.saturating_add(1);
        }
    }
    Ok((blocks, used_neon))
}

fn vertical_wide_block(
    intermediate: &[i32],
    row_width: usize,
    output_x: usize,
    count: usize,
    first_source_y: usize,
    weights: &[f64],
) -> Result<[i32; LANES], PilError> {
    let mut sums = f64x8::splat(0.0);
    for (tap, &weight) in weights.iter().enumerate() {
        let source_y = first_source_y.checked_add(tap).ok_or_else(invalid_plan)?;
        let row_start = source_y.checked_mul(row_width).ok_or_else(invalid_plan)?;
        let row = intermediate
            .get(row_start..row_start.checked_add(row_width).ok_or_else(invalid_plan)?)
            .ok_or_else(invalid_plan)?;
        let lanes = std::array::from_fn(|lane| {
            if lane < count {
                f64::from(row[output_x + lane])
            } else {
                0.0
            }
        });
        sums = f64x8::splat(weight).mul_add(f64x8::new(lanes), sums);
    }
    Ok(sums.to_array().map(|value| round_up(value) as i32))
}

fn write_output_block(
    output: &mut [u8],
    output_row: usize,
    output_x: usize,
    count: usize,
    samples: &[i32; LANES],
) -> Result<(), PilError> {
    let start = output_row
        .checked_add(output_x.checked_mul(4).ok_or_else(invalid_plan)?)
        .ok_or_else(invalid_plan)?;
    let byte_count = count.checked_mul(4).ok_or_else(invalid_plan)?;
    let bytes = output
        .get_mut(start..start.checked_add(byte_count).ok_or_else(invalid_plan)?)
        .ok_or_else(invalid_plan)?;
    for (destination, sample) in bytes.chunks_exact_mut(4).zip(samples.iter()) {
        destination.copy_from_slice(&sample.to_le_bytes());
    }
    Ok(())
}

fn record_path(blocks: u64, used_neon: bool) {
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path(if used_neon {
        "neon-f64-resize"
    } else {
        "vector"
    });
}

fn invalid_plan() -> PilError {
    PilError::InternalError("ImageI32 NEON Resize plan did not match its source layout".into())
}
