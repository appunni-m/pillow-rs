//! Boxed byte convolution kernel for SIMD Resize and ImageOps.fit.

#[cfg(feature = "parallel")]
use super::super::{SIMD_RESIZE_LANES, simd_rows_mut_serial};
use super::super::{
    compact_resize_coeffs_to_source_span, precompute_coeffs_boxed_for_filter, simd_unsupported,
};
use super::u8_convolution::{
    resize_horizontal_vector_row, resize_premultiply_u8, resize_unpremultiply_u8,
    resize_vertical_vector_row,
};
use super::{ResizeHorizontalPlan, build_resize_horizontal_plan};
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::FilterCoeffs;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

/// Resize a fractional source box with the native SIMD two-pass resampler.
///
/// The box coordinates are scalar control data only. Coefficients are built
/// by the same Pillow-compatible fixed-point builder used by
/// `pil_resize_boxed`; the row kernels then vectorize the pixel accumulations
/// without creating a cropped source image or retrying through CPU.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_convolution_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    parallel_pixel_threshold: usize,
) -> Result<DynamicImage, PilError> {
    let plan = boxed_resize_plan(
        img,
        output_width,
        output_height,
        box_left,
        box_top,
        box_right,
        box_bottom,
        filter,
        channels,
        premultiplied_alpha,
    )?;
    let mut intermediate = vec![0u8; plan.intermediate_len];
    let horizontal_counts = boxed_resize_horizontal_stage(
        img.as_bytes(),
        &plan,
        &mut intermediate,
        parallel_pixel_threshold,
    )?;
    let mut output = vec![0u8; plan.output_len];
    let vertical_counts =
        boxed_resize_vertical_stage(&intermediate, &plan, &mut output, parallel_pixel_threshold)?;
    let (vector_blocks, scalar_tail) =
        boxed_resize_telemetry(&plan, horizontal_counts, vertical_counts);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        plan.output_width as u32,
        plan.output_height as u32,
        output,
        plan.channels,
    )?;
    Ok(preserve_mode(img, result))
}

struct BoxedResizePlan {
    output_width: usize,
    output_height: usize,
    output_len: usize,
    intermediate_len: usize,
    first_source_row: usize,
    intermediate_rows: usize,
    source_stride: usize,
    intermediate_stride: usize,
    channels: usize,
    premultiplied_alpha: bool,
    need_horizontal: bool,
    need_vertical: bool,
    horizontal: FilterCoeffs,
    vertical: FilterCoeffs,
}

fn boxed_resize_plan(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
) -> Result<BoxedResizePlan, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Fit"))?;
    let output_len = output_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD Fit source buffer shape mismatch".into(),
        ));
    }
    let horizontal = precompute_coeffs_boxed_for_filter(
        output_width as u32,
        source_width as u32,
        box_left,
        box_right,
        filter,
    );
    let mut vertical = precompute_coeffs_boxed_for_filter(
        output_height as u32,
        source_height as u32,
        box_top,
        box_bottom,
        filter,
    );
    let box_left = box_left as f32;
    let box_top = box_top as f32;
    let box_right = box_right as f32;
    let box_bottom = box_bottom as f32;
    let need_horizontal =
        output_width != source_width || box_left != 0.0 || box_right != output_width as f32;
    let need_vertical =
        output_height != source_height || box_top != 0.0 || box_bottom != output_height as f32;
    let (first_source_row, intermediate_rows) = if need_vertical {
        compact_resize_coeffs_to_source_span(&mut vertical, source_height as u32)
            .map(|(first, count)| (first as usize, count as usize))
            .unwrap_or((0, source_height))
    } else {
        (0, source_height)
    };
    let intermediate_len = intermediate_rows
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    Ok(BoxedResizePlan {
        output_width,
        output_height,
        output_len,
        intermediate_len,
        first_source_row,
        intermediate_rows,
        source_stride,
        intermediate_stride,
        channels,
        premultiplied_alpha,
        need_horizontal,
        need_vertical,
        horizontal,
        vertical,
    })
}

fn boxed_resize_horizontal_stage(
    source: &[u8],
    plan: &BoxedResizePlan,
    intermediate: &mut [u8],
    parallel_pixel_threshold: usize,
) -> Result<(u64, u64), PilError> {
    if !plan.need_horizontal {
        boxed_horizontal_identity(
            source,
            intermediate,
            plan.source_stride,
            plan.intermediate_len,
            plan.first_source_row,
            plan.premultiplied_alpha,
            plan.channels,
        )?;
        return Ok((0, 0));
    }
    let horizontal_plan =
        build_resize_horizontal_plan(&plan.horizontal, plan.output_width, plan.channels, false)
            .ok_or_else(|| simd_unsupported("Fit"))?;
    let rows = BoxedHorizontalRows {
        source,
        coeffs: &plan.horizontal,
        plan: &horizontal_plan,
        source_stride: plan.source_stride,
        intermediate_stride: plan.intermediate_stride,
        first_source_row: plan.first_source_row,
        channels: plan.channels,
        output_width: plan.output_width,
        premultiplied_alpha: plan.premultiplied_alpha,
    };
    boxed_horizontal_pass(
        &rows,
        intermediate,
        plan.intermediate_rows,
        parallel_pixel_threshold,
    )
}

fn boxed_resize_vertical_stage(
    intermediate: &[u8],
    plan: &BoxedResizePlan,
    output: &mut [u8],
    parallel_pixel_threshold: usize,
) -> Result<(u64, u64), PilError> {
    if !plan.need_vertical {
        boxed_vertical_identity(
            intermediate,
            output,
            plan.channels,
            plan.premultiplied_alpha,
        );
        return Ok((0, 0));
    }
    let rows = BoxedVerticalRows {
        intermediate,
        coeffs: &plan.vertical,
        intermediate_rows: plan.intermediate_rows,
        output_stride: plan.intermediate_stride,
        output_width: plan.output_width,
        channels: plan.channels,
        premultiplied_alpha: plan.premultiplied_alpha,
    };
    boxed_vertical_pass(&rows, output, plan.output_height, parallel_pixel_threshold)
}

fn boxed_resize_telemetry(
    plan: &BoxedResizePlan,
    horizontal: (u64, u64),
    vertical: (u64, u64),
) -> (u64, u64) {
    #[cfg(feature = "parallel")]
    {
        let vector_width = if plan.output_width < SIMD_RESIZE_LANES {
            SIMD_RESIZE_LANES
        } else {
            plan.output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
        };
        let blocks_per_row = vector_width.div_ceil(SIMD_RESIZE_LANES) as u64;
        let tail_per_row = plan.output_width.saturating_sub(vector_width) as u64;
        let _ = (horizontal, vertical);
        (
            blocks_per_row
                .saturating_mul(plan.intermediate_rows as u64)
                .saturating_add(blocks_per_row.saturating_mul(plan.output_height as u64)),
            tail_per_row
                .saturating_mul(plan.intermediate_rows as u64)
                .saturating_add(tail_per_row.saturating_mul(plan.output_height as u64)),
        )
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = plan;
        let (horizontal_blocks, horizontal_tail) = horizontal;
        let (vertical_blocks, vertical_tail) = vertical;
        (
            horizontal_blocks.saturating_add(vertical_blocks),
            horizontal_tail.saturating_add(vertical_tail),
        )
    }
}

struct BoxedHorizontalRows<'a> {
    source: &'a [u8],
    coeffs: &'a FilterCoeffs,
    plan: &'a ResizeHorizontalPlan,
    source_stride: usize,
    intermediate_stride: usize,
    first_source_row: usize,
    channels: usize,
    output_width: usize,
    premultiplied_alpha: bool,
}

struct BoxedVerticalRows<'a> {
    intermediate: &'a [u8],
    coeffs: &'a FilterCoeffs,
    intermediate_rows: usize,
    output_stride: usize,
    output_width: usize,
    channels: usize,
    premultiplied_alpha: bool,
}

fn boxed_horizontal_pass(
    rows: &BoxedHorizontalRows<'_>,
    intermediate: &mut [u8],
    intermediate_rows: usize,
    parallel_pixel_threshold: usize,
) -> Result<(u64, u64), PilError> {
    #[cfg(feature = "parallel")]
    {
        if intermediate_rows.saturating_mul(rows.output_width) < parallel_pixel_threshold {
            boxed_horizontal_counted_rows(rows, intermediate, intermediate_rows).map(|_| (0, 0))
        } else {
            boxed_horizontal_serial_rows(rows, intermediate, intermediate_rows).map(|_| (0, 0))
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = parallel_pixel_threshold;
        boxed_horizontal_counted_rows(rows, intermediate, intermediate_rows)
    }
}

fn boxed_horizontal_counted_rows(
    rows: &BoxedHorizontalRows<'_>,
    intermediate: &mut [u8],
    intermediate_rows: usize,
) -> Result<(u64, u64), PilError> {
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row_index in 0..intermediate_rows {
        let source_y = rows
            .first_source_row
            .checked_add(row_index)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let intermediate_start = row_index
            .checked_mul(rows.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let output_row = intermediate
            .get_mut(intermediate_start..intermediate_start + rows.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let (blocks, tail) = boxed_horizontal_row(rows, source_y, output_row)?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

#[cfg(feature = "parallel")]
fn boxed_horizontal_serial_rows(
    rows: &BoxedHorizontalRows<'_>,
    intermediate: &mut [u8],
    intermediate_rows: usize,
) -> Result<(), PilError> {
    let failed = AtomicBool::new(false);
    simd_rows_mut_serial!(
        intermediate,
        rows.intermediate_stride,
        intermediate_rows,
        |row_start, row_end, row_index, output_row| {
            let _ = (row_start, row_end);
            let source_y = rows.first_source_row.saturating_add(row_index as usize);
            if boxed_horizontal_row(rows, source_y, output_row).is_err() {
                failed.store(true, Ordering::Relaxed);
            }
        }
    );
    if failed.load(Ordering::Relaxed) {
        return Err(simd_unsupported("Fit"));
    }
    Ok(())
}

fn boxed_horizontal_row(
    rows: &BoxedHorizontalRows<'_>,
    source_y: usize,
    output_row: &mut [u8],
) -> Result<(u64, u64), PilError> {
    let source_start = source_y
        .checked_mul(rows.source_stride)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_row = rows
        .source
        .get(source_start..source_start + rows.source_stride)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    resize_horizontal_vector_row(
        source_row,
        rows.channels,
        rows.coeffs,
        rows.plan,
        rows.output_width,
        output_row,
        rows.premultiplied_alpha,
    )
    .ok_or_else(|| simd_unsupported("Fit"))
}

fn boxed_horizontal_identity(
    source: &[u8],
    intermediate: &mut [u8],
    source_stride: usize,
    intermediate_len: usize,
    first_source_row: usize,
    premultiplied_alpha: bool,
    channels: usize,
) -> Result<(), PilError> {
    let source_start = first_source_row
        .checked_mul(source_stride)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_end = source_start
        .checked_add(intermediate_len)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source = source
        .get(source_start..source_end)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if premultiplied_alpha {
        for (source_pixel, intermediate_pixel) in source
            .chunks_exact(channels)
            .zip(intermediate.chunks_exact_mut(channels))
        {
            let alpha = source_pixel[channels - 1];
            for channel in 0..channels {
                intermediate_pixel[channel] = if channel == channels - 1 {
                    alpha
                } else {
                    resize_premultiply_u8(source_pixel[channel], alpha)
                };
            }
        }
    } else {
        intermediate.copy_from_slice(source);
    }
    Ok(())
}

fn boxed_vertical_pass(
    rows: &BoxedVerticalRows<'_>,
    output: &mut [u8],
    output_height: usize,
    parallel_pixel_threshold: usize,
) -> Result<(u64, u64), PilError> {
    #[cfg(feature = "parallel")]
    {
        if rows.output_width.saturating_mul(output_height) < parallel_pixel_threshold {
            boxed_vertical_counted_rows(rows, output, output_height).map(|_| (0, 0))
        } else {
            boxed_vertical_serial_rows(rows, output, output_height).map(|_| (0, 0))
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = parallel_pixel_threshold;
        boxed_vertical_counted_rows(rows, output, output_height)
    }
}

fn boxed_vertical_counted_rows(
    rows: &BoxedVerticalRows<'_>,
    output: &mut [u8],
    output_height: usize,
) -> Result<(u64, u64), PilError> {
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_y in 0..output_height {
        let output_start = output_y
            .checked_mul(rows.output_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let output_row = output
            .get_mut(output_start..output_start + rows.output_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let (blocks, tail) = boxed_vertical_row(rows, output_y, output_row)?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

#[cfg(feature = "parallel")]
fn boxed_vertical_serial_rows(
    rows: &BoxedVerticalRows<'_>,
    output: &mut [u8],
    output_height: usize,
) -> Result<(), PilError> {
    let failed = AtomicBool::new(false);
    simd_rows_mut_serial!(
        output,
        rows.output_stride,
        output_height,
        |row_start, row_end, output_y, output_row| {
            let _ = (row_start, row_end);
            if boxed_vertical_row(rows, output_y as usize, output_row).is_err() {
                failed.store(true, Ordering::Relaxed);
            }
        }
    );
    if failed.load(Ordering::Relaxed) {
        return Err(simd_unsupported("Fit"));
    }
    Ok(())
}

fn boxed_vertical_row(
    rows: &BoxedVerticalRows<'_>,
    output_y: usize,
    output_row: &mut [u8],
) -> Result<(u64, u64), PilError> {
    resize_vertical_vector_row(
        rows.intermediate,
        rows.output_width,
        rows.intermediate_rows,
        rows.channels,
        rows.coeffs,
        output_y,
        output_row,
        rows.premultiplied_alpha,
        false,
    )
    .ok_or_else(|| simd_unsupported("Fit"))
}

fn boxed_vertical_identity(
    intermediate: &[u8],
    output: &mut [u8],
    channels: usize,
    premultiplied_alpha: bool,
) {
    if premultiplied_alpha {
        for (intermediate_pixel, output_pixel) in intermediate
            .chunks_exact(channels)
            .zip(output.chunks_exact_mut(channels))
        {
            let alpha = intermediate_pixel[channels - 1];
            for channel in 0..channels {
                output_pixel[channel] = if channel == channels - 1 {
                    alpha
                } else {
                    resize_unpremultiply_u8(intermediate_pixel[channel], alpha)
                };
            }
        }
    } else {
        let output_len = output.len();
        output.copy_from_slice(&intermediate[..output_len]);
    }
}
