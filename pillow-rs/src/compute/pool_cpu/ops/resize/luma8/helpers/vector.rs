//! Shared fixed-point Luma8 kernel for architecture-selected vector strategies.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, fixed_point_to_u8_i32,
};
use crate::error::PilError;
use crate::ops::pil_resize::{FilterCoeffs, precompute_coeffs};
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;
use wide::i32x8;

const LANES: usize = 8;

struct Tap {
    source_bases: [usize; LANES],
    weights: [i32; LANES],
}

struct Block {
    taps: Vec<Tap>,
}

struct HorizontalPlan {
    vector_width: usize,
    blocks: Vec<Block>,
}

struct VerticalTail<'a> {
    intermediate: &'a [u8],
    output_width: usize,
    source_height: usize,
    coefficients: &'a FilterCoeffs,
    output_y: usize,
    source_y: usize,
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<GrayImage, PilError> {
    let output_dims = CheckedDims::new(width, height, 1)?;
    let mut output = vec![0; output_dims.total_bytes()];
    execute_into(input, width, height, filter, &mut output)?;
    GrayImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::ValueError("resize_l: failed to create output buffer".into()))
}

/// Writes the exact Luma8 SSE2 strategy into a caller-owned full-width window.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_into_window(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    if matches!(filter, ResampleFilter::Nearest)
        || width == 0
        || height == 0
        || input.width() == 0
        || input.height() == 0
    {
        return false;
    }
    execute_into(input, width, height, filter, output).is_ok()
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn execute_into(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (source_width, source_height) = input.dimensions();
    let source_dims = CheckedDims::new(source_width, source_height, 1)?;
    let intermediate_dims = CheckedDims::new(width, source_height, 1)?;
    let output_dims = CheckedDims::new(width, height, 1)?;
    if input.as_raw().len() != source_dims.total_bytes()
        || output.len() != output_dims.total_bytes()
    {
        return Err(PilError::InternalError(
            "ImageLuma8 SSE2 Resize received an invalid source or output window".into(),
        ));
    }

    let horizontal = precompute_coeffs(width, source_width, filter);
    let vertical = precompute_coeffs(height, source_height, filter);
    validate_coefficients(&horizontal, &vertical, source_width, source_height)?;
    let output_width = usize::try_from(width)
        .map_err(|_| PilError::DimensionError("ImageLuma8 Resize width is too large".into()))?;
    let source_width = usize::try_from(source_width)
        .map_err(|_| PilError::DimensionError("ImageLuma8 source width is too large".into()))?;
    let source_height = usize::try_from(source_height)
        .map_err(|_| PilError::DimensionError("ImageLuma8 source height is too large".into()))?;
    let output_height = usize::try_from(height)
        .map_err(|_| PilError::DimensionError("ImageLuma8 Resize height is too large".into()))?;
    let plan = build_horizontal_plan(&horizontal, output_width).ok_or_else(invalid_kernel_plan)?;
    let mut intermediate = vec![0; intermediate_dims.total_bytes()];
    let (horizontal_blocks, horizontal_tail) = horizontal_pass(
        input.as_raw(),
        source_width,
        source_height,
        output_width,
        &horizontal,
        &plan,
        &mut intermediate,
    )
    .ok_or_else(invalid_kernel_plan)?;
    let (vertical_blocks, vertical_tail) = vertical_pass(
        &intermediate,
        output_width,
        source_height,
        output_height,
        &vertical,
        output,
    )
    .ok_or_else(invalid_kernel_plan)?;
    record_vector_work(
        horizontal_blocks,
        vertical_blocks,
        horizontal_tail,
        vertical_tail,
    );
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn validate_coefficients(
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    source_width: u32,
    source_height: u32,
) -> Result<(), PilError> {
    let fits = usize::try_from(source_width).ok().is_some_and(|width| {
        byte_coefficients_fit_source(horizontal, width) && byte_coefficients_fit_i32(horizontal)
    }) && usize::try_from(source_height).ok().is_some_and(|height| {
        byte_coefficients_fit_source(vertical, height) && byte_coefficients_fit_i32(vertical)
    });
    if fits {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "selected ImageLuma8 SSE2 Resize received unsupported coefficients".into(),
        ))
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn build_horizontal_plan(
    coefficients: &FilterCoeffs,
    output_width: usize,
) -> Option<HorizontalPlan> {
    let vector_width = if output_width < LANES {
        LANES
    } else {
        output_width / LANES * LANES
    };
    let mut blocks = Vec::with_capacity(vector_width.div_ceil(LANES));
    for output_x in (0..vector_width).step_by(LANES) {
        blocks.push(build_horizontal_block(coefficients, output_x)?);
    }
    Some(HorizontalPlan {
        vector_width,
        blocks,
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn build_horizontal_block(coefficients: &FilterCoeffs, output_x: usize) -> Option<Block> {
    let max_count = (0..LANES)
        .filter_map(|lane| output_x.checked_add(lane))
        .filter_map(|index| coefficients.count.get(index))
        .copied()
        .max()
        .unwrap_or(0);
    let mut taps = Vec::with_capacity(max_count);
    for tap in 0..max_count {
        taps.push(build_horizontal_tap(coefficients, output_x, tap)?);
    }
    Some(Block { taps })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn build_horizontal_tap(coefficients: &FilterCoeffs, output_x: usize, tap: usize) -> Option<Tap> {
    let mut source_bases = [0; LANES];
    let mut weights = [0; LANES];
    for lane in 0..LANES {
        let index = output_x.checked_add(lane)?;
        let Some(&count) = coefficients.count.get(index) else {
            continue;
        };
        if tap >= count {
            continue;
        }
        let source_x = usize::try_from(*coefficients.xmin.get(index)?)
            .ok()?
            .checked_add(tap)?;
        let coefficient_index = coefficients.offsets.get(index)?.checked_add(tap)?;
        source_bases[lane] = source_x;
        weights[lane] = i32::try_from(*coefficients.weights.get(coefficient_index)?).ok()?;
    }
    Some(Tap {
        source_bases,
        weights,
    })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_pass(
    source: &[u8],
    source_width: usize,
    source_height: usize,
    output_width: usize,
    coefficients: &FilterCoeffs,
    plan: &HorizontalPlan,
    intermediate: &mut [u8],
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for source_y in 0..source_height {
        let source_start = source_y.checked_mul(source_width)?;
        let output_start = source_y.checked_mul(output_width)?;
        let source_row = source.get(source_start..source_start.checked_add(source_width)?)?;
        let output_row =
            intermediate.get_mut(output_start..output_start.checked_add(output_width)?)?;
        let (row_blocks, row_tail) = horizontal_row(source_row, coefficients, plan, output_row)?;
        vector_blocks = vector_blocks.saturating_add(row_blocks);
        scalar_tail = scalar_tail.saturating_add(row_tail);
    }
    Some((vector_blocks, scalar_tail))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_row(
    source: &[u8],
    coefficients: &FilterCoeffs,
    plan: &HorizontalPlan,
    output: &mut [u8],
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0_u64;
    for (block_index, output_x) in (0..plan.vector_width).step_by(LANES).enumerate() {
        let mut sum = i32x8::splat(0);
        for tap in &plan.blocks.get(block_index)?.taps {
            let mut samples = [0; LANES];
            for lane in 0..LANES {
                if tap.weights[lane] != 0 {
                    samples[lane] = i32::from(*source.get(tap.source_bases[lane])?);
                }
            }
            sum += i32x8::new(samples) * i32x8::new(tap.weights);
        }
        let result = sum.to_array().map(fixed_point_to_u8_i32);
        for (lane, &sample) in result.iter().enumerate() {
            if let Some(destination) = output.get_mut(output_x.checked_add(lane)?) {
                *destination = sample;
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = horizontal_scalar_tail(source, coefficients, plan.vector_width, output)?;
    Some((vector_blocks, scalar_tail))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_scalar_tail(
    source: &[u8],
    coefficients: &FilterCoeffs,
    start: usize,
    output: &mut [u8],
) -> Option<u64> {
    let mut scalar_tail = 0_u64;
    for output_x in start..output.len() {
        let weights = coefficient_slice(coefficients, output_x)?;
        let source_x = usize::try_from(*coefficients.xmin.get(output_x)?).ok()?;
        let mut sum = 0_i32;
        for (tap, &weight) in weights.iter().enumerate() {
            let index = source_x.checked_add(tap)?;
            sum += i32::from(*source.get(index)?) * i32::try_from(weight).ok()?;
        }
        output[output_x] = fixed_point_to_u8_i32(sum);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some(scalar_tail)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_pass(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) -> Option<(u64, u64)> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for output_y in 0..output_height {
        let output_start = output_y.checked_mul(output_width)?;
        let output_row = output.get_mut(output_start..output_start.checked_add(output_width)?)?;
        let (row_blocks, row_tail) = vertical_row(
            intermediate,
            output_width,
            source_height,
            coefficients,
            output_y,
            output_row,
        )?;
        vector_blocks = vector_blocks.saturating_add(row_blocks);
        scalar_tail = scalar_tail.saturating_add(row_tail);
    }
    Some((vector_blocks, scalar_tail))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    coefficients: &FilterCoeffs,
    output_y: usize,
    output: &mut [u8],
) -> Option<(u64, u64)> {
    let weights = coefficient_slice(coefficients, output_y)?;
    let source_y = usize::try_from(*coefficients.xmin.get(output_y)?).ok()?;
    let vector_width = if output_width < LANES {
        LANES
    } else {
        output_width / LANES * LANES
    };
    let mut vector_blocks = 0_u64;
    for output_x in (0..vector_width).step_by(LANES) {
        let active = output_width.saturating_sub(output_x).min(LANES);
        let mut sum = i32x8::splat(0);
        for (tap, &weight) in weights.iter().enumerate() {
            let row = source_y.checked_add(tap)?;
            if row >= source_height {
                return None;
            }
            let row_start = row.checked_mul(output_width)?;
            let source_start = row_start.checked_add(output_x)?;
            let mut samples = [0; LANES];
            for lane in 0..active {
                samples[lane] = i32::from(*intermediate.get(source_start.checked_add(lane)?)?);
            }
            let weight = i32::try_from(weight).ok()?;
            sum += i32x8::new(samples) * i32x8::splat(weight);
        }
        let result = sum.to_array().map(fixed_point_to_u8_i32);
        for (lane, &sample) in result.iter().take(active).enumerate() {
            *output.get_mut(output_x.checked_add(lane)?)? = sample;
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = vertical_scalar_tail(
        VerticalTail {
            intermediate,
            output_width,
            source_height,
            coefficients,
            output_y,
            source_y,
        },
        vector_width,
        output,
    )?;
    Some((vector_blocks, scalar_tail))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_scalar_tail(context: VerticalTail<'_>, start: usize, output: &mut [u8]) -> Option<u64> {
    let weights = coefficient_slice(context.coefficients, context.output_y)?;
    let mut scalar_tail = 0_u64;
    for output_x in start..context.output_width {
        let mut sum = 0_i32;
        for (tap, &weight) in weights.iter().enumerate() {
            let row = context.source_y.checked_add(tap)?;
            if row >= context.source_height {
                return None;
            }
            let index = row
                .checked_mul(context.output_width)?
                .checked_add(output_x)?;
            sum += i32::from(*context.intermediate.get(index)?) * i32::try_from(weight).ok()?;
        }
        output[output_x] = fixed_point_to_u8_i32(sum);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some(scalar_tail)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn coefficient_slice(coefficients: &FilterCoeffs, index: usize) -> Option<&[i64]> {
    let start = *coefficients.offsets.get(index)?;
    let count = *coefficients.count.get(index)?;
    coefficients.weights.get(start..start.checked_add(count)?)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn record_vector_work(
    horizontal_blocks: u64,
    vertical_blocks: u64,
    horizontal_tail: u64,
    vertical_tail: u64,
) {
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks.saturating_add(vertical_blocks),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_tail.saturating_add(vertical_tail),
    );
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn invalid_kernel_plan() -> PilError {
    PilError::InternalError("selected ImageLuma8 SSE2 Resize kernel rejected its plan".into())
}
