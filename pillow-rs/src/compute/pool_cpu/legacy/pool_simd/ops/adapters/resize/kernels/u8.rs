//! Native-byte SIMD separable convolution kernels for Resize and Thumbnail.

#[cfg(feature = "parallel")]
use super::super::SIMD_RESIZE_LANES;
#[cfg(feature = "parallel")]
use super::super::simd_rows_mut_serial;
use super::super::simd_unsupported;
use super::u8_convolution::{resize_horizontal_convolution_row, resize_vertical_vector_row};
use super::u8_cover_i32::resize_vertical_cover_i32_vector_row;
use super::{
    ResizeHorizontalPlan, build_resize_horizontal_plan, resize_u8_coefficients_fit_i32,
    resize_vertical_row_touches_nonzero_source,
};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::{FilterCoeffs, precompute_coeffs};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicBool, Ordering};

pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_convolution(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
) -> Result<DynamicImage, PilError> {
    simd_resize_convolution_with_parallel_threshold(
        img,
        output_width,
        output_height,
        filter,
        channels,
        premultiplied_alpha,
        cover_byte_i32,
        0,
        false,
    )
}

pub(super) fn simd_resize_convolution_with_parallel_threshold(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
) -> Result<DynamicImage, PilError> {
    simd_resize_convolution_with_zero_row_elision(
        img,
        output_width,
        output_height,
        filter,
        channels,
        premultiplied_alpha,
        cover_byte_i32,
        parallel_pixel_threshold,
        vectorize_tail,
        false,
    )
}

pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_convolution_with_zero_row_elision(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
    skip_zero_source_rows: bool,
) -> Result<DynamicImage, PilError> {
    let output_dims = CheckedDims::new_allow_empty(output_width, output_height, channels as u8)?;
    let mut output = output_dims.alloc_buffer();
    simd_resize_convolution_into_with_zero_row_elision(
        img,
        output_width,
        output_height,
        filter,
        channels,
        premultiplied_alpha,
        cover_byte_i32,
        parallel_pixel_threshold,
        vectorize_tail,
        skip_zero_source_rows,
        &mut output,
    )?;
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Run the existing two-pass SIMD resize into a caller-owned contiguous
/// output window. ImageOps.pad uses this only when the contain raster spans
/// the entire destination width, so the window has the ordinary resize row
/// stride while sitting inside the final canvas.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_convolution_into(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
    output: &mut [u8],
) -> Result<(), PilError> {
    simd_resize_convolution_into_with_zero_row_elision(
        img,
        output_width,
        output_height,
        filter,
        channels,
        premultiplied_alpha,
        cover_byte_i32,
        parallel_pixel_threshold,
        vectorize_tail,
        false,
        output,
    )
}

fn simd_resize_convolution_into_with_zero_row_elision(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
    skip_zero_source_rows: bool,
    output: &mut [u8],
) -> Result<(), PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let intermediate_len = source_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let output_len = output_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "SIMD resize output window shape mismatch".into(),
        ));
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD resize source buffer shape mismatch".into(),
        ));
    }
    let horizontal = precompute_coeffs(output_width as u32, source_width as u32, filter);
    let vertical = precompute_coeffs(output_height as u32, source_height as u32, filter);
    let cover_byte_i32 = cover_byte_i32
        && matches!(
            (channels, premultiplied_alpha),
            (1, false) | (2, true) | (3, false) | (4, false) | (4, true)
        )
        && resize_u8_coefficients_fit_i32(&horizontal)
        && resize_u8_coefficients_fit_i32(&vertical);
    let horizontal_plan =
        build_resize_horizontal_plan(&horizontal, output_width, channels, vectorize_tail)
            .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let context = ResizeU8Convolution {
        source: img.as_bytes(),
        horizontal: &horizontal,
        vertical: &vertical,
        horizontal_plan: &horizontal_plan,
        source_height,
        output_width,
        output_height,
        channels,
        source_stride,
        intermediate_stride,
        premultiplied_alpha,
        cover_byte_i32,
        parallel_pixel_threshold,
        vectorize_tail,
        skip_zero_source_rows,
    };
    let mut intermediate = vec![0u8; intermediate_len];
    let mut nonzero_rows = if skip_zero_source_rows {
        vec![false; source_height]
    } else {
        Vec::new()
    };
    let mut premultiplied_source_row = if premultiplied_alpha {
        vec![0u8; source_stride]
    } else {
        Vec::new()
    };
    let horizontal_stats = resize_u8_horizontal_pass(
        &context,
        &mut intermediate,
        &mut nonzero_rows,
        &mut premultiplied_source_row,
    )?;
    let vertical_stats = resize_u8_vertical_pass(&context, &intermediate, &nonzero_rows, output)?;
    #[cfg(feature = "parallel")]
    let (vector_blocks, scalar_tail) = resize_u8_parallel_receipts(
        &context,
        horizontal_stats.vector_rows,
        vertical_stats.vector_rows,
    );
    #[cfg(not(feature = "parallel"))]
    let (vector_blocks, scalar_tail) = (
        horizontal_stats
            .vector_blocks
            .saturating_add(vertical_stats.vector_blocks),
        horizontal_stats
            .scalar_tail
            .saturating_add(vertical_stats.scalar_tail),
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(())
}

struct ResizeU8Convolution<'a> {
    source: &'a [u8],
    horizontal: &'a FilterCoeffs,
    vertical: &'a FilterCoeffs,
    horizontal_plan: &'a ResizeHorizontalPlan,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    channels: usize,
    source_stride: usize,
    intermediate_stride: usize,
    premultiplied_alpha: bool,
    cover_byte_i32: bool,
    parallel_pixel_threshold: usize,
    vectorize_tail: bool,
    skip_zero_source_rows: bool,
}

#[derive(Default)]
struct ResizeU8PassStats {
    #[cfg(not(feature = "parallel"))]
    vector_blocks: u64,
    #[cfg(not(feature = "parallel"))]
    scalar_tail: u64,
    #[cfg(feature = "parallel")]
    vector_rows: u64,
}

fn resize_u8_horizontal_pass(
    context: &ResizeU8Convolution<'_>,
    intermediate: &mut [u8],
    nonzero_rows: &mut [bool],
    premultiplied_source_row: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    #[cfg(feature = "parallel")]
    {
        resize_u8_horizontal_parallel(
            context,
            intermediate,
            nonzero_rows,
            premultiplied_source_row,
        )
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = context.parallel_pixel_threshold;
        resize_u8_horizontal_serial(
            context,
            intermediate,
            nonzero_rows,
            premultiplied_source_row,
        )
    }
}

#[cfg(not(feature = "parallel"))]
fn resize_u8_horizontal_serial(
    context: &ResizeU8Convolution<'_>,
    intermediate: &mut [u8],
    nonzero_rows: &mut [bool],
    premultiplied_source_row: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    let mut stats = ResizeU8PassStats::default();
    for source_y in 0..context.source_height {
        let output_start = source_y
            .checked_mul(context.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = intermediate
            .get_mut(output_start..output_start + context.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let (blocks, tail, processed) = resize_u8_horizontal_row(
            context,
            source_y,
            output_row,
            nonzero_rows,
            premultiplied_source_row,
        )
        .ok_or_else(|| simd_unsupported("Resize"))?;
        stats.vector_blocks = stats.vector_blocks.saturating_add(blocks);
        stats.scalar_tail = stats.scalar_tail.saturating_add(tail);
        let _ = processed;
    }
    Ok(stats)
}

#[cfg(feature = "parallel")]
fn resize_u8_horizontal_parallel(
    context: &ResizeU8Convolution<'_>,
    intermediate: &mut [u8],
    nonzero_rows: &mut [bool],
    premultiplied_source_row: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    let mut stats = ResizeU8PassStats::default();
    if context.source_height.saturating_mul(context.output_width) < context.parallel_pixel_threshold
    {
        for source_y in 0..context.source_height {
            let output_start = source_y
                .checked_mul(context.intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = intermediate
                .get_mut(output_start..output_start + context.intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let (_, _, processed) = resize_u8_horizontal_row(
                context,
                source_y,
                output_row,
                nonzero_rows,
                premultiplied_source_row,
            )
            .ok_or_else(|| simd_unsupported("Resize"))?;
            stats.vector_rows = stats.vector_rows.saturating_add(u64::from(processed));
        }
        return Ok(stats);
    }
    let failed = AtomicBool::new(false);
    simd_rows_mut_serial!(
        intermediate,
        context.intermediate_stride,
        context.source_height,
        |row_start, row_end, source_y, output_row| {
            let _ = (row_start, row_end);
            if resize_u8_horizontal_row(
                context,
                source_y as usize,
                output_row,
                nonzero_rows,
                premultiplied_source_row,
            )
            .is_none()
            {
                failed.store(true, Ordering::Relaxed);
            } else if !context.skip_zero_source_rows || nonzero_rows[source_y as usize] {
                stats.vector_rows = stats.vector_rows.saturating_add(1);
            }
        }
    );
    if failed.load(Ordering::Relaxed) {
        return Err(simd_unsupported("Resize"));
    }
    Ok(stats)
}

fn resize_u8_horizontal_row(
    context: &ResizeU8Convolution<'_>,
    source_y: usize,
    intermediate_row: &mut [u8],
    nonzero_rows: &mut [bool],
    premultiplied_source_row: &mut [u8],
) -> Option<(u64, u64, bool)> {
    let source_start = source_y.checked_mul(context.source_stride)?;
    let source_end = source_start.checked_add(context.source_stride)?;
    let source_row = context.source.get(source_start..source_end)?;
    if context.skip_zero_source_rows && source_row.iter().all(|&sample| sample == 0) {
        intermediate_row.fill(0);
        return Some((0, 0, false));
    }
    let (blocks, tail) = resize_horizontal_convolution_row(
        source_row,
        context.channels,
        context.horizontal,
        context.horizontal_plan,
        context.output_width,
        intermediate_row,
        context.premultiplied_alpha,
        context.cover_byte_i32,
        premultiplied_source_row,
    )?;
    if context.skip_zero_source_rows {
        *nonzero_rows.get_mut(source_y)? = true;
    }
    Some((blocks, tail, true))
}

fn resize_u8_vertical_pass(
    context: &ResizeU8Convolution<'_>,
    intermediate: &[u8],
    nonzero_rows: &[bool],
    output: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    #[cfg(feature = "parallel")]
    {
        resize_u8_vertical_parallel(context, intermediate, nonzero_rows, output)
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = context.parallel_pixel_threshold;
        resize_u8_vertical_serial(context, intermediate, nonzero_rows, output)
    }
}

#[cfg(not(feature = "parallel"))]
fn resize_u8_vertical_serial(
    context: &ResizeU8Convolution<'_>,
    intermediate: &[u8],
    nonzero_rows: &[bool],
    output: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    let mut stats = ResizeU8PassStats::default();
    for output_y in 0..context.output_height {
        let output_start = output_y
            .checked_mul(context.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output
            .get_mut(output_start..output_start + context.intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let (blocks, tail, processed) =
            resize_u8_vertical_row(context, intermediate, nonzero_rows, output_y, output_row)
                .ok_or_else(|| simd_unsupported("Resize"))?;
        stats.vector_blocks = stats.vector_blocks.saturating_add(blocks);
        stats.scalar_tail = stats.scalar_tail.saturating_add(tail);
        let _ = processed;
    }
    Ok(stats)
}

#[cfg(feature = "parallel")]
fn resize_u8_vertical_parallel(
    context: &ResizeU8Convolution<'_>,
    intermediate: &[u8],
    nonzero_rows: &[bool],
    output: &mut [u8],
) -> Result<ResizeU8PassStats, PilError> {
    let mut stats = ResizeU8PassStats::default();
    if context.output_width.saturating_mul(context.output_height) < context.parallel_pixel_threshold
    {
        for output_y in 0..context.output_height {
            let output_start = output_y
                .checked_mul(context.intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output
                .get_mut(output_start..output_start + context.intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let (_, _, processed) =
                resize_u8_vertical_row(context, intermediate, nonzero_rows, output_y, output_row)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
            stats.vector_rows = stats.vector_rows.saturating_add(u64::from(processed));
        }
        return Ok(stats);
    }
    let failed = AtomicBool::new(false);
    simd_rows_mut_serial!(
        output,
        context.intermediate_stride,
        context.output_height,
        |row_start, row_end, output_y, output_row| {
            let _ = (row_start, row_end);
            match resize_u8_vertical_row(
                context,
                intermediate,
                nonzero_rows,
                output_y as usize,
                output_row,
            ) {
                Some((_, _, true)) => stats.vector_rows = stats.vector_rows.saturating_add(1),
                Some((_, _, false)) => {}
                None => failed.store(true, Ordering::Relaxed),
            }
        }
    );
    if failed.load(Ordering::Relaxed) {
        return Err(simd_unsupported("Resize"));
    }
    Ok(stats)
}

fn resize_u8_vertical_row(
    context: &ResizeU8Convolution<'_>,
    intermediate: &[u8],
    nonzero_rows: &[bool],
    output_y: usize,
    output_row: &mut [u8],
) -> Option<(u64, u64, bool)> {
    if context.skip_zero_source_rows
        && !resize_vertical_row_touches_nonzero_source(
            context.vertical,
            context.source_height,
            nonzero_rows,
            output_y,
        )?
    {
        return Some((0, 0, false));
    }
    let result = if context.cover_byte_i32 {
        resize_vertical_cover_i32_vector_row(
            intermediate,
            context.output_width,
            context.source_height,
            context.channels,
            context.vertical,
            output_y,
            output_row,
            context.premultiplied_alpha,
        )
    } else {
        resize_vertical_vector_row(
            intermediate,
            context.output_width,
            context.source_height,
            context.channels,
            context.vertical,
            output_y,
            output_row,
            context.premultiplied_alpha,
            context.vectorize_tail,
        )
    }?;
    Some((result.0, result.1, true))
}

#[cfg(feature = "parallel")]
fn resize_u8_parallel_receipts(
    context: &ResizeU8Convolution<'_>,
    horizontal_vector_rows: u64,
    vertical_vector_rows: u64,
) -> (u64, u64) {
    let blocks_per_row = if context.output_width < SIMD_RESIZE_LANES {
        1
    } else if context.vectorize_tail {
        context.output_width.div_ceil(SIMD_RESIZE_LANES)
    } else {
        context.output_width / SIMD_RESIZE_LANES
    };
    let blocks_per_row = blocks_per_row as u64;
    let scalar_tail_per_row = if context.output_width < SIMD_RESIZE_LANES || context.vectorize_tail
    {
        0
    } else {
        (context.output_width % SIMD_RESIZE_LANES) as u64
    };
    let vector_blocks = blocks_per_row
        .saturating_mul(horizontal_vector_rows)
        .saturating_add(blocks_per_row.saturating_mul(vertical_vector_rows));
    let scalar_tail = scalar_tail_per_row
        .saturating_mul(horizontal_vector_rows)
        .saturating_add(scalar_tail_per_row.saturating_mul(vertical_vector_rows));
    (vector_blocks, scalar_tail)
}
