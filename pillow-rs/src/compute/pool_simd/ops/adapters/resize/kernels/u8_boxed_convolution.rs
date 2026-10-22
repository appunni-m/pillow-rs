//! Boxed byte convolution kernel for SIMD Resize and ImageOps.fit.

#[cfg(feature = "parallel")]
use super::super::{SIMD_RESIZE_LANES, simd_rows_mut_serial};
use super::super::{
    compact_resize_coeffs_to_source_span, precompute_coeffs_boxed_for_filter,
    resize_horizontal_vector_row, resize_premultiply_u8, resize_unpremultiply_u8,
    resize_vertical_vector_row, simd_unsupported,
};
use super::build_resize_horizontal_plan;
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::PilError;
use crate::image::preserve_mode;
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
    // Every SIMD build uses the same serial row scheduler. The threshold
    // selects the vectorized route only when the Parallel CPU feature exists.
    let _ = parallel_pixel_threshold;
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
    let box_left_f32 = box_left as f32;
    let box_top_f32 = box_top as f32;
    let box_right_f32 = box_right as f32;
    let box_bottom_f32 = box_bottom as f32;
    let need_horizontal =
        output_width != source_width || box_left_f32 != 0.0 || box_right_f32 != output_width as f32;
    let need_vertical = output_height != source_height
        || box_top_f32 != 0.0
        || box_bottom_f32 != output_height as f32;
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
    let horizontal_plan = if need_horizontal {
        Some(
            build_resize_horizontal_plan(&horizontal, output_width, channels, false)
                .ok_or_else(|| simd_unsupported("Fit"))?,
        )
    } else {
        None
    };
    let mut intermediate = vec![0u8; intermediate_len];
    #[cfg(feature = "parallel")]
    let vector_blocks;
    #[cfg(feature = "parallel")]
    let scalar_tail;
    #[cfg(not(feature = "parallel"))]
    let mut vector_blocks = 0u64;
    #[cfg(not(feature = "parallel"))]
    let mut scalar_tail = 0u64;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source = img.as_bytes();
    if need_horizontal {
        let horizontal_plan = horizontal_plan
            .as_ref()
            .ok_or_else(|| simd_unsupported("Fit"))?;
        #[cfg(feature = "parallel")]
        if intermediate_rows.saturating_mul(output_width) < parallel_pixel_threshold {
            for row_index in 0..intermediate_rows {
                let source_y = first_source_row.saturating_add(row_index);
                let source_start = source_y
                    .checked_mul(source_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                let intermediate_start = row_index
                    .checked_mul(intermediate_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                let source_row = source
                    .get(source_start..source_start + source_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                let intermediate_row = intermediate
                    .get_mut(intermediate_start..intermediate_start + intermediate_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                resize_horizontal_vector_row(
                    source_row,
                    channels,
                    &horizontal,
                    horizontal_plan,
                    output_width,
                    intermediate_row,
                    premultiplied_alpha,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            }
        } else {
            #[cfg(feature = "parallel")]
            {
                let failed = AtomicBool::new(false);
                simd_rows_mut_serial!(
                    &mut intermediate,
                    intermediate_stride,
                    intermediate_rows,
                    |row_start, row_end, row_index, intermediate_row| {
                        let _ = (row_start, row_end);
                        let source_y = first_source_row.saturating_add(row_index as usize);
                        let source_start = source_y.saturating_mul(source_stride);
                        let source_end = source_start.saturating_add(source_stride);
                        let Some(source_row) = source.get(source_start..source_end) else {
                            failed.store(true, Ordering::Relaxed);
                            return;
                        };
                        if resize_horizontal_vector_row(
                            source_row,
                            channels,
                            &horizontal,
                            horizontal_plan,
                            output_width,
                            intermediate_row,
                            premultiplied_alpha,
                        )
                        .is_none()
                        {
                            failed.store(true, Ordering::Relaxed);
                        }
                    }
                );
                if failed.load(Ordering::Relaxed) {
                    return Err(simd_unsupported("Fit"));
                }
            }
        }
        #[cfg(not(feature = "parallel"))]
        for row_index in 0..intermediate_rows {
            let source_y = first_source_row.saturating_add(row_index);
            let source_start = source_y
                .checked_mul(source_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let intermediate_start = row_index
                .checked_mul(intermediate_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let source_row = source
                .get(source_start..source_start + source_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let intermediate_row = intermediate
                .get_mut(intermediate_start..intermediate_start + intermediate_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let (blocks, tail) = resize_horizontal_vector_row(
                source_row,
                channels,
                &horizontal,
                horizontal_plan,
                output_width,
                intermediate_row,
                premultiplied_alpha,
            )
            .ok_or_else(|| simd_unsupported("Fit"))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    } else if premultiplied_alpha {
        let source_start = first_source_row
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let source_end = source_start
            .checked_add(intermediate_len)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        for (source_pixel, intermediate_pixel) in source
            .get(source_start..source_end)
            .ok_or_else(|| simd_unsupported("Fit"))?
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
        let source_start = first_source_row
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let source_end = source_start
            .checked_add(intermediate_len)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        intermediate.copy_from_slice(
            source
                .get(source_start..source_end)
                .ok_or_else(|| simd_unsupported("Fit"))?,
        );
    }

    let mut output = vec![0u8; output_len];
    let output_stride = intermediate_stride;
    if need_vertical {
        #[cfg(feature = "parallel")]
        if output_width.saturating_mul(output_height) < parallel_pixel_threshold {
            for output_y in 0..output_height {
                let output_start = output_y
                    .checked_mul(output_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                let output_row = output
                    .get_mut(output_start..output_start + output_stride)
                    .ok_or_else(|| simd_unsupported("Fit"))?;
                resize_vertical_vector_row(
                    &intermediate,
                    output_width,
                    intermediate_rows,
                    channels,
                    &vertical,
                    output_y,
                    output_row,
                    premultiplied_alpha,
                    false,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            }
        } else {
            #[cfg(feature = "parallel")]
            {
                let failed = AtomicBool::new(false);
                simd_rows_mut_serial!(
                    &mut output,
                    output_stride,
                    output_height,
                    |row_start, row_end, output_y, output_row| {
                        let _ = (row_start, row_end);
                        if resize_vertical_vector_row(
                            &intermediate,
                            output_width,
                            intermediate_rows,
                            channels,
                            &vertical,
                            output_y as usize,
                            output_row,
                            premultiplied_alpha,
                            false,
                        )
                        .is_none()
                        {
                            failed.store(true, Ordering::Relaxed);
                        }
                    }
                );
                if failed.load(Ordering::Relaxed) {
                    return Err(simd_unsupported("Fit"));
                }
            }
        }
        #[cfg(not(feature = "parallel"))]
        for output_y in 0..output_height {
            let output_start = output_y
                .checked_mul(output_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let output_row = output
                .get_mut(output_start..output_start + output_stride)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let (blocks, tail) = resize_vertical_vector_row(
                &intermediate,
                output_width,
                intermediate_rows,
                channels,
                &vertical,
                output_y,
                output_row,
                premultiplied_alpha,
                false,
            )
            .ok_or_else(|| simd_unsupported("Fit"))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    } else if premultiplied_alpha {
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
    #[cfg(feature = "parallel")]
    {
        let vector_width = if output_width < SIMD_RESIZE_LANES {
            SIMD_RESIZE_LANES
        } else {
            output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
        };
        let horizontal_blocks = vector_width.div_ceil(SIMD_RESIZE_LANES) as u64;
        let horizontal_tail = output_width.saturating_sub(vector_width) as u64;
        vector_blocks = horizontal_blocks
            .saturating_mul(intermediate_rows as u64)
            .saturating_add(horizontal_blocks.saturating_mul(output_height as u64));
        scalar_tail = horizontal_tail
            .saturating_mul(intermediate_rows as u64)
            .saturating_add(horizontal_tail.saturating_mul(output_height as u64));
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}
