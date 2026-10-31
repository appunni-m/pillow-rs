//! Boxed native-byte vector convolution for exact Resize mode strategies.

use super::convolution::{
    resize_horizontal_vector_row, resize_premultiply_alpha_row, resize_unpremultiply_u8,
    resize_vertical_vector_row,
};
use super::plan::{ResizeHorizontalPlan, build_resize_horizontal_plan};
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source,
};
use crate::error::PilError;
use crate::ops::pil_resize::{
    FilterCoeffs, compact_resize_coeffs_to_source_span, precompute_coeffs_boxed_for_filter,
};
use crate::pipeline::ResampleFilter;

pub(super) fn execute<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    if matches!(filter, ResampleFilter::Nearest) {
        return Err(invalid_boxed_plan());
    }
    validate_channels::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>()?;
    let plan = BoxedResizePlan::new::<CHANNELS>(
        source,
        source_size,
        destination_size,
        bounds,
        filter,
    )?;
    let mut intermediate = plan.intermediate_dims.alloc_buffer();
    let mut output = plan.output_dims.alloc_buffer();
    let mut premultiplied_row = if PREMULTIPLY_STRAIGHT_ALPHA {
        vec![0; plan.source_stride]
    } else {
        Vec::new()
    };
    let horizontal = horizontal_stage::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        source,
        &plan,
        &mut intermediate,
        &mut premultiplied_row,
    )?;
    let vertical = vertical_stage::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        &intermediate,
        &plan,
        &mut output,
    )?;
    super::telemetry::record_vector_work(horizontal.0, vertical.0, horizontal.1, vertical.1);
    Ok(output)
}

fn validate_channels<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
) -> Result<(), PilError> {
    if (1..=4).contains(&CHANNELS)
        && (!PREMULTIPLY_STRAIGHT_ALPHA || matches!(CHANNELS, 2 | 4))
    {
        Ok(())
    } else {
        Err(PilError::DimensionError(
            "boxed native-byte Resize requires one to four channels and a valid alpha layout"
                .into(),
        ))
    }
}

struct BoxedResizePlan {
    output_width: usize,
    output_height: usize,
    source_stride: usize,
    intermediate_stride: usize,
    first_source_row: usize,
    intermediate_rows: usize,
    need_horizontal: bool,
    need_vertical: bool,
    horizontal: FilterCoeffs,
    vertical: FilterCoeffs,
    horizontal_plan: Option<ResizeHorizontalPlan>,
    intermediate_dims: CheckedDims,
    output_dims: CheckedDims,
}

impl BoxedResizePlan {
    fn new<const CHANNELS: usize>(
        source: &[u8],
        source_size: (u32, u32),
        destination_size: (u32, u32),
        bounds: (f64, f64, f64, f64),
        filter: ResampleFilter,
    ) -> Result<Self, PilError> {
        let source_dims = CheckedDims::new(source_size.0, source_size.1, CHANNELS as u8)?;
        let output_dims = CheckedDims::new(destination_size.0, destination_size.1, CHANNELS as u8)?;
        if source.len() != source_dims.total_bytes() {
            return Err(PilError::InternalError(
                "boxed native-byte Resize received an invalid source buffer".into(),
            ));
        }
        let bounds = narrow_valid_bounds(source_size, bounds).ok_or_else(invalid_boxed_plan)?;
        let horizontal = precompute_coeffs_boxed_for_filter(
            destination_size.0,
            source_size.0,
            bounds.0,
            bounds.2,
            filter,
        );
        let mut vertical = precompute_coeffs_boxed_for_filter(
            destination_size.1,
            source_size.1,
            bounds.1,
            bounds.3,
            filter,
        );
        validate_coefficients(&horizontal, &vertical, source_size)?;
        let need_horizontal = horizontal_stage_needed(source_size.0, destination_size.0, bounds);
        let need_vertical = vertical_stage_needed(source_size.1, destination_size.1, bounds);
        let (first_source_row, intermediate_rows) = compact_vertical_coefficients(
            &mut vertical,
            source_size.1,
            need_vertical,
        );
        let intermediate_width = usize::try_from(destination_size.0)
            .map_err(|_| invalid_boxed_plan())?;
        let intermediate_height = u32::try_from(intermediate_rows)
            .map_err(|_| invalid_boxed_plan())?;
        let intermediate_dims =
            CheckedDims::new(destination_size.0, intermediate_height, CHANNELS as u8)?;
        let intermediate_stride = intermediate_width
            .checked_mul(CHANNELS)
            .ok_or_else(invalid_boxed_plan)?;
        let source_stride = usize::try_from(source_size.0)
            .ok()
            .and_then(|width| width.checked_mul(CHANNELS))
            .ok_or_else(invalid_boxed_plan)?;
        let horizontal_plan = if need_horizontal {
            Some(
                build_resize_horizontal_plan::<CHANNELS>(&horizontal, intermediate_width)
                    .ok_or_else(invalid_boxed_plan)?,
            )
        } else {
            None
        };
        Ok(Self {
            output_width: intermediate_width,
            output_height: usize::try_from(destination_size.1)
                .map_err(|_| invalid_boxed_plan())?,
            source_stride,
            intermediate_stride,
            first_source_row: usize::try_from(first_source_row)
                .map_err(|_| invalid_boxed_plan())?,
            intermediate_rows,
            need_horizontal,
            need_vertical,
            horizontal,
            vertical,
            horizontal_plan,
            intermediate_dims,
            output_dims,
        })
    }
}

fn narrow_valid_bounds(
    source_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    let bounds = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if !bounds.0.is_finite()
        || !bounds.1.is_finite()
        || !bounds.2.is_finite()
        || !bounds.3.is_finite()
        || bounds.0 < 0.0
        || bounds.1 < 0.0
        || bounds.2 > f64::from(source_size.0 as f32)
        || bounds.3 > f64::from(source_size.1 as f32)
        || bounds.2 < bounds.0
        || bounds.3 < bounds.1
    {
        None
    } else {
        Some(bounds)
    }
}

fn validate_coefficients(
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
    source_size: (u32, u32),
) -> Result<(), PilError> {
    let source_width = usize::try_from(source_size.0).map_err(|_| invalid_boxed_plan())?;
    let source_height = usize::try_from(source_size.1).map_err(|_| invalid_boxed_plan())?;
    if byte_coefficients_fit_source(horizontal, source_width)
        && byte_coefficients_fit_source(vertical, source_height)
        && byte_coefficients_fit_i32(horizontal)
        && byte_coefficients_fit_i32(vertical)
    {
        Ok(())
    } else {
        Err(invalid_boxed_plan())
    }
}

fn horizontal_stage_needed(
    source_width: u32,
    output_width: u32,
    bounds: (f64, f64, f64, f64),
) -> bool {
    output_width != source_width || bounds.0 != 0.0 || bounds.2 != f64::from(output_width as f32)
}

fn vertical_stage_needed(
    source_height: u32,
    output_height: u32,
    bounds: (f64, f64, f64, f64),
) -> bool {
    output_height != source_height
        || bounds.1 != 0.0
        || bounds.3 != f64::from(output_height as f32)
}

fn compact_vertical_coefficients(
    vertical: &mut FilterCoeffs,
    source_height: u32,
    need_vertical: bool,
) -> (u32, usize) {
    if need_vertical {
        compact_resize_coeffs_to_source_span(vertical, source_height)
            .map(|(first, count)| (first, count as usize))
            .unwrap_or((0, source_height as usize))
    } else {
        (0, source_height as usize)
    }
}

struct HorizontalRows<'a, const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool> {
    source: &'a [u8],
    coefficients: &'a FilterCoeffs,
    plan: Option<&'a ResizeHorizontalPlan>,
    source_stride: usize,
    intermediate_stride: usize,
    first_source_row: usize,
    output_width: usize,
}

fn horizontal_stage<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    source: &[u8],
    resize: &BoxedResizePlan,
    intermediate: &mut [u8],
    premultiplied_row: &mut [u8],
) -> Result<(u64, u64), PilError> {
    if !resize.need_horizontal {
        copy_horizontal_identity::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
            source,
            resize,
            intermediate,
        )?;
        return Ok((0, 0));
    }
    let rows = HorizontalRows::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA> {
        source,
        coefficients: &resize.horizontal,
        plan: resize.horizontal_plan.as_ref(),
        source_stride: resize.source_stride,
        intermediate_stride: resize.intermediate_stride,
        first_source_row: resize.first_source_row,
        output_width: resize.output_width,
    };
    process_horizontal_rows(&rows, resize.intermediate_rows, intermediate, premultiplied_row)
}

fn process_horizontal_rows<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    rows: &HorizontalRows<'_, CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>,
    row_count: usize,
    intermediate: &mut [u8],
    premultiplied_row: &mut [u8],
) -> Result<(u64, u64), PilError> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for row_index in 0..row_count {
        let source_y = rows
            .first_source_row
            .checked_add(row_index)
            .ok_or_else(invalid_boxed_plan)?;
        let output_start = row_index
            .checked_mul(rows.intermediate_stride)
            .ok_or_else(invalid_boxed_plan)?;
        let output_end = output_start
            .checked_add(rows.intermediate_stride)
            .ok_or_else(invalid_boxed_plan)?;
        let output_row = intermediate
            .get_mut(output_start..output_end)
            .ok_or_else(invalid_boxed_plan)?;
        let (blocks, tail) = boxed_horizontal_row(rows, source_y, output_row, premultiplied_row)?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

fn boxed_horizontal_row<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    rows: &HorizontalRows<'_, CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>,
    source_y: usize,
    output_row: &mut [u8],
    premultiplied_row: &mut [u8],
) -> Result<(u64, u64), PilError> {
    let source_start = source_y
        .checked_mul(rows.source_stride)
        .ok_or_else(invalid_boxed_plan)?;
    let source_end = source_start
        .checked_add(rows.source_stride)
        .ok_or_else(invalid_boxed_plan)?;
    let source_row = rows
        .source
        .get(source_start..source_end)
        .ok_or_else(invalid_boxed_plan)?;
    let source_row = if PREMULTIPLY_STRAIGHT_ALPHA {
        resize_premultiply_alpha_row::<CHANNELS>(source_row, premultiplied_row)
            .ok_or_else(invalid_boxed_plan)?;
        &*premultiplied_row
    } else {
        source_row
    };
    resize_horizontal_vector_row::<CHANNELS>(
        source_row,
        rows.coefficients,
        rows.plan.ok_or_else(invalid_boxed_plan)?,
        rows.output_width,
        output_row,
    )
    .ok_or_else(invalid_boxed_plan)
}

fn copy_horizontal_identity<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    source: &[u8],
    resize: &BoxedResizePlan,
    intermediate: &mut [u8],
) -> Result<(), PilError> {
    let source_start = resize
        .first_source_row
        .checked_mul(resize.source_stride)
        .ok_or_else(invalid_boxed_plan)?;
    let source_end = source_start
        .checked_add(intermediate.len())
        .ok_or_else(invalid_boxed_plan)?;
    let source = source.get(source_start..source_end).ok_or_else(invalid_boxed_plan)?;
    if PREMULTIPLY_STRAIGHT_ALPHA {
        for (source_pixel, output_pixel) in source
            .chunks_exact(CHANNELS)
            .zip(intermediate.chunks_exact_mut(CHANNELS))
        {
            let alpha = source_pixel[CHANNELS - 1];
            for channel in 0..CHANNELS {
                output_pixel[channel] = if channel == CHANNELS - 1 {
                    alpha
                } else {
                    super::convolution::resize_premultiply_u8(source_pixel[channel], alpha)
                };
            }
        }
    } else {
        intermediate.copy_from_slice(source);
    }
    Ok(())
}

struct VerticalRows<'a> {
    intermediate: &'a [u8],
    coefficients: &'a FilterCoeffs,
    output_width: usize,
    source_height: usize,
    output_stride: usize,
}

fn vertical_stage<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    intermediate: &[u8],
    resize: &BoxedResizePlan,
    output: &mut [u8],
) -> Result<(u64, u64), PilError> {
    if !resize.need_vertical {
        copy_vertical_identity::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(intermediate, output)?;
        return Ok((0, 0));
    }
    let rows = VerticalRows {
        intermediate,
        coefficients: &resize.vertical,
        output_width: resize.output_width,
        source_height: resize.intermediate_rows,
        output_stride: resize.output_dims.row_stride(),
    };
    process_vertical_rows::<CHANNELS, PREMULTIPLY_STRAIGHT_ALPHA>(
        &rows,
        resize.output_height,
        output,
    )
}

fn process_vertical_rows<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    rows: &VerticalRows<'_>,
    output_height: usize,
    output: &mut [u8],
) -> Result<(u64, u64), PilError> {
    let mut vector_blocks = 0_u64;
    let mut scalar_tail = 0_u64;
    for output_y in 0..output_height {
        let output_start = output_y
            .checked_mul(rows.output_stride)
            .ok_or_else(invalid_boxed_plan)?;
        let output_end = output_start
            .checked_add(rows.output_stride)
            .ok_or_else(invalid_boxed_plan)?;
        let output_row = output
            .get_mut(output_start..output_end)
            .ok_or_else(invalid_boxed_plan)?;
        let (blocks, tail) = resize_vertical_vector_row::<
            CHANNELS,
            PREMULTIPLY_STRAIGHT_ALPHA,
        >(
            rows.intermediate,
            rows.output_width,
            rows.source_height,
            rows.coefficients,
            output_y,
            output_row,
        )
        .ok_or_else(invalid_boxed_plan)?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

fn copy_vertical_identity<const CHANNELS: usize, const PREMULTIPLY_STRAIGHT_ALPHA: bool>(
    intermediate: &[u8],
    output: &mut [u8],
) -> Result<(), PilError> {
    if intermediate.len() != output.len() {
        return Err(invalid_boxed_plan());
    }
    if PREMULTIPLY_STRAIGHT_ALPHA {
        for (intermediate_pixel, output_pixel) in intermediate
            .chunks_exact(CHANNELS)
            .zip(output.chunks_exact_mut(CHANNELS))
        {
            let alpha = intermediate_pixel[CHANNELS - 1];
            for channel in 0..CHANNELS {
                output_pixel[channel] = if channel == CHANNELS - 1 {
                    alpha
                } else {
                    resize_unpremultiply_u8(intermediate_pixel[channel], alpha)
                };
            }
        }
    } else {
        output.copy_from_slice(intermediate);
    }
    Ok(())
}

fn invalid_boxed_plan() -> PilError {
    PilError::InternalError("selected native-byte boxed Resize kernel rejected its plan".into())
}
