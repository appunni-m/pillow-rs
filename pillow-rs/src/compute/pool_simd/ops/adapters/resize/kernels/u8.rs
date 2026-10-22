//! Native-byte SIMD separable convolution kernels for Resize and Thumbnail.

#[cfg(feature = "parallel")]
use super::super::SIMD_RESIZE_LANES;
#[cfg(feature = "parallel")]
use super::super::simd_rows_mut_serial;
use super::super::simd_unsupported;
use super::u8_convolution::{resize_horizontal_convolution_row, resize_vertical_vector_row};
use super::u8_cover_i32::resize_vertical_cover_i32_vector_row;
use super::{
    build_resize_horizontal_plan, resize_u8_coefficients_fit_i32,
    resize_vertical_row_touches_nonzero_source,
};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::precompute_coeffs;
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
    let mut intermediate = vec![0u8; intermediate_len];
    #[cfg(feature = "parallel")]
    let vector_blocks;
    #[cfg(feature = "parallel")]
    let scalar_tail;
    #[cfg(not(feature = "parallel"))]
    let mut vector_blocks = 0u64;
    #[cfg(not(feature = "parallel"))]
    let mut scalar_tail = 0u64;
    #[cfg(not(feature = "parallel"))]
    let _ = parallel_pixel_threshold;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = img.as_bytes();
    let mut horizontal_nonzero_rows = if skip_zero_source_rows {
        vec![false; source_height]
    } else {
        Vec::new()
    };
    let mut premultiplied_source_row = if premultiplied_alpha {
        vec![0u8; source_stride]
    } else {
        Vec::new()
    };
    #[cfg(feature = "parallel")]
    let mut horizontal_vector_rows = 0u64;
    #[cfg(feature = "parallel")]
    if source_height.saturating_mul(output_width) < parallel_pixel_threshold {
        for source_y in 0..source_height {
            let source_start = source_y
                .checked_mul(source_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let intermediate_start = source_y
                .checked_mul(intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let source_row = source
                .get(source_start..source_start + source_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let intermediate_row = intermediate
                .get_mut(intermediate_start..intermediate_start + intermediate_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            if skip_zero_source_rows && source_row.iter().all(|&sample| sample == 0) {
                intermediate_row.fill(0);
                continue;
            }
            resize_horizontal_convolution_row(
                source_row,
                channels,
                &horizontal,
                &horizontal_plan,
                output_width,
                intermediate_row,
                premultiplied_alpha,
                cover_byte_i32,
                &mut premultiplied_source_row,
            )
            .ok_or_else(|| simd_unsupported("Resize"))?;
            if skip_zero_source_rows {
                horizontal_nonzero_rows[source_y] = true;
            }
            horizontal_vector_rows = horizontal_vector_rows.saturating_add(1);
        }
    } else {
        // Horizontal rows are independent once the coefficient table is
        // built. Keep the same per-row tap order and row-exclusive writes;
        // the SIMD path remains serial while Parallel CPU is selected through
        // the CPU backend.
        // on large downscales.
        let failed = AtomicBool::new(false);
        simd_rows_mut_serial!(
            &mut intermediate,
            intermediate_stride,
            source_height,
            |row_start, row_end, y, intermediate_row| {
                let _ = (row_start, row_end);
                let source_start = (y as usize).saturating_mul(source_stride);
                let source_end = source_start.saturating_add(source_stride);
                let Some(source_row) = source.get(source_start..source_end) else {
                    failed.store(true, Ordering::Relaxed);
                    return;
                };
                if skip_zero_source_rows && source_row.iter().all(|&sample| sample == 0) {
                    intermediate_row.fill(0);
                    return;
                }
                let row_result = resize_horizontal_convolution_row(
                    source_row,
                    channels,
                    &horizontal,
                    &horizontal_plan,
                    output_width,
                    intermediate_row,
                    premultiplied_alpha,
                    cover_byte_i32,
                    &mut premultiplied_source_row,
                );
                if row_result.is_none() {
                    failed.store(true, Ordering::Relaxed);
                } else {
                    if skip_zero_source_rows {
                        horizontal_nonzero_rows[y as usize] = true;
                    }
                    horizontal_vector_rows = horizontal_vector_rows.saturating_add(1);
                }
            }
        );
        if failed.load(Ordering::Relaxed) {
            return Err(simd_unsupported("Resize"));
        }
    }
    #[cfg(not(feature = "parallel"))]
    for source_y in 0..source_height {
        let source_start = source_y
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_start = source_y
            .checked_mul(intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let source_row = source
            .get(source_start..source_start + source_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_row = intermediate
            .get_mut(intermediate_start..intermediate_start + intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        if skip_zero_source_rows && source_row.iter().all(|&sample| sample == 0) {
            intermediate_row.fill(0);
            continue;
        }
        let row_result = resize_horizontal_convolution_row(
            source_row,
            channels,
            &horizontal,
            &horizontal_plan,
            output_width,
            intermediate_row,
            premultiplied_alpha,
            cover_byte_i32,
            &mut premultiplied_source_row,
        );
        row_result
            .map(|(blocks, tail)| {
                if skip_zero_source_rows {
                    horizontal_nonzero_rows[source_y] = true;
                }
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            })
            .ok_or_else(|| simd_unsupported("Resize"))?;
    }
    let output_stride = intermediate_stride;
    #[cfg(feature = "parallel")]
    let mut vertical_vector_rows = 0u64;
    let vertical_row_has_nonzero_source = |output_y| {
        if skip_zero_source_rows {
            resize_vertical_row_touches_nonzero_source(
                &vertical,
                source_height,
                &horizontal_nonzero_rows,
                output_y,
            )
        } else {
            Some(true)
        }
    };
    #[cfg(feature = "parallel")]
    if output_width.saturating_mul(output_height) < parallel_pixel_threshold {
        for output_y in 0..output_height {
            let output_start = output_y
                .checked_mul(output_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output
                .get_mut(output_start..output_start + output_stride)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            match vertical_row_has_nonzero_source(output_y) {
                Some(true) => {}
                Some(false) => continue,
                None => return Err(simd_unsupported("Resize")),
            }
            let row_result = if cover_byte_i32 {
                resize_vertical_cover_i32_vector_row(
                    &intermediate,
                    output_width,
                    source_height,
                    channels,
                    &vertical,
                    output_y,
                    output_row,
                    premultiplied_alpha,
                )
            } else {
                resize_vertical_vector_row(
                    &intermediate,
                    output_width,
                    source_height,
                    channels,
                    &vertical,
                    output_y,
                    output_row,
                    premultiplied_alpha,
                    vectorize_tail,
                )
            };
            row_result.ok_or_else(|| simd_unsupported("Resize"))?;
            vertical_vector_rows = vertical_vector_rows.saturating_add(1);
        }
    } else {
        let failed = AtomicBool::new(false);
        simd_rows_mut_serial!(
            output,
            output_stride,
            output_height,
            |row_start, row_end, output_y, output_row| {
                let _ = (row_start, row_end);
                let output_y = output_y as usize;
                match vertical_row_has_nonzero_source(output_y) {
                    Some(true) => {}
                    Some(false) => return,
                    None => {
                        failed.store(true, Ordering::Relaxed);
                        return;
                    }
                }
                let row_result = if cover_byte_i32 {
                    resize_vertical_cover_i32_vector_row(
                        &intermediate,
                        output_width,
                        source_height,
                        channels,
                        &vertical,
                        output_y,
                        output_row,
                        premultiplied_alpha,
                    )
                } else {
                    resize_vertical_vector_row(
                        &intermediate,
                        output_width,
                        source_height,
                        channels,
                        &vertical,
                        output_y,
                        output_row,
                        premultiplied_alpha,
                        vectorize_tail,
                    )
                };
                if row_result.is_none() {
                    failed.store(true, Ordering::Relaxed);
                } else {
                    vertical_vector_rows = vertical_vector_rows.saturating_add(1);
                }
            }
        );
        if failed.load(Ordering::Relaxed) {
            return Err(simd_unsupported("Resize"));
        }
    }
    #[cfg(not(feature = "parallel"))]
    let _ = parallel_pixel_threshold;
    #[cfg(not(feature = "parallel"))]
    for output_y in 0..output_height {
        let output_start = output_y
            .checked_mul(output_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output
            .get_mut(output_start..output_start + output_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        match vertical_row_has_nonzero_source(output_y) {
            Some(true) => {}
            Some(false) => continue,
            None => return Err(simd_unsupported("Resize")),
        }
        let row_result = if cover_byte_i32 {
            resize_vertical_cover_i32_vector_row(
                &intermediate,
                output_width,
                source_height,
                channels,
                &vertical,
                output_y,
                output_row,
                premultiplied_alpha,
            )
        } else {
            resize_vertical_vector_row(
                &intermediate,
                output_width,
                source_height,
                channels,
                &vertical,
                output_y,
                output_row,
                premultiplied_alpha,
                vectorize_tail,
            )
        };
        row_result
            .map(|(blocks, tail)| {
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            })
            .ok_or_else(|| simd_unsupported("Resize"))?;
    }
    #[cfg(feature = "parallel")]
    {
        // The vector/tail counters are deterministic functions of the output
        // width and row count.  Compute them after parallel writes rather
        // than contending on shared counters inside the hot loops.
        let blocks_per_row = if output_width < SIMD_RESIZE_LANES {
            1
        } else if vectorize_tail {
            output_width.div_ceil(SIMD_RESIZE_LANES)
        } else {
            output_width / SIMD_RESIZE_LANES
        } as u64;
        let scalar_tail_per_row = if output_width < SIMD_RESIZE_LANES || vectorize_tail {
            0
        } else {
            output_width % SIMD_RESIZE_LANES
        } as u64;
        let computed_vector_blocks = blocks_per_row
            .saturating_mul(horizontal_vector_rows)
            .saturating_add(blocks_per_row.saturating_mul(vertical_vector_rows));
        let computed_scalar_tail = scalar_tail_per_row
            .saturating_mul(horizontal_vector_rows)
            .saturating_add(scalar_tail_per_row.saturating_mul(vertical_vector_rows));
        vector_blocks = computed_vector_blocks;
        scalar_tail = computed_scalar_tail;
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(())
}
