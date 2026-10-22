//! Rotate-family native kernels selected by the SIMD adapter.

mod kernels;

#[cfg(not(feature = "parallel"))]
pub(super) use self::kernels::copy_contiguous_rgb_runs;

use self::kernels::{
    native_identity_rotate, simd_bilinear_rotate_native, simd_empty_rotate_native,
    simd_nearest_rotate_native, simd_right_angle_rotate_native, simd_typed_bilinear_rotate_native,
};
use super::{rotate_identity_contract, rotate_uses_discrete_fast_path, simd_unsupported};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::DynamicImage;
use wide::u8x16;

#[inline(always)]
pub(super) fn copy_nearest_affine_pixels(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    destination_width: usize,
    destination_height: usize,
    channels: usize,
    pixels_per_vector: usize,
    block_bytes: usize,
    origin_x: i64,
    origin_y: i64,
    step_x_x: i64,
    step_y_x: i64,
    step_x_y: i64,
    step_y_y: i64,
    fill: (u8, u8, u8, u8),
) -> (u64, u64) {
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let output_row_bytes = destination_width * channels;
    for (destination_y, output_row) in output
        .chunks_exact_mut(output_row_bytes)
        .take(destination_height)
        .enumerate()
    {
        let mut source_x = origin_x + destination_y as i64 * step_y_x;
        let mut source_y = origin_y + destination_y as i64 * step_y_y;
        let mut destination_x = 0usize;
        while destination_x + pixels_per_vector <= destination_width {
            let mut block = [0u8; 16];
            for pixel in 0..pixels_per_vector {
                let input_x = source_x >> 16;
                let input_y = source_y >> 16;
                let block_start = pixel * channels;
                if input_x >= 0 && input_x < width as i64 && input_y >= 0 && input_y < height as i64
                {
                    let source_start = (input_y as usize * width + input_x as usize) * channels;
                    block[block_start..block_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] =
                            super::rotate_fill_sample(fill, channels, channel);
                    }
                }
                source_x += step_x_x;
                source_y += step_x_y;
            }
            let output_start = destination_x * channels;
            output_row[output_start..output_start + block_bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += pixels_per_vector;
        }
        while destination_x < destination_width {
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let output_start = destination_x * channels;
            if input_x >= 0 && input_x < width as i64 && input_y >= 0 && input_y < height as i64 {
                let source_start = (input_y as usize * width + input_x as usize) * channels;
                output_row[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    output_row[output_start + channel] =
                        super::rotate_fill_sample(fill, channels, channel);
                }
            }
            source_x += step_x_x;
            source_y += step_x_y;
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    (vector_blocks, scalar_tail)
}

#[inline(always)]
pub(super) fn copy_right_angle_pixels(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    width_i32: i32,
    height_i32: i32,
    degree: u32,
    x_offset: i32,
    y_offset: i32,
    fill: (u8, u8, u8, u8),
) -> (u64, u64) {
    let pixels_per_vector = (super::SIMD_RANK_FILTER_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    let row_bytes = width * channels;
    let vector_width = width / pixels_per_vector * pixels_per_vector;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for destination_y in 0..height {
        let output_row = destination_y * row_bytes;
        let mut destination_x = 0usize;
        while destination_x < vector_width {
            let mut block = [0u8; 16];
            for pixel in 0..pixels_per_vector {
                let x = destination_x + pixel;
                let (source_x, source_y) = if degree == 90 {
                    (
                        width_i32 - 1 - destination_y as i32 + y_offset,
                        x as i32 - x_offset,
                    )
                } else {
                    (
                        destination_y as i32 - y_offset,
                        height_i32 - 1 - x as i32 + x_offset,
                    )
                };
                let block_start = pixel * channels;
                if source_x >= 0 && source_x < width_i32 && source_y >= 0 && source_y < height_i32 {
                    let source_start = (source_y as usize * width + source_x as usize) * channels;
                    block[block_start..block_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] =
                            super::rotate_fill_sample(fill, channels, channel);
                    }
                }
            }
            let output_start = output_row + destination_x * channels;
            output[output_start..output_start + block_bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += pixels_per_vector;
        }
        while destination_x < width {
            let x = destination_x;
            let (source_x, source_y) = if degree == 90 {
                (
                    width_i32 - 1 - destination_y as i32 + y_offset,
                    x as i32 - x_offset,
                )
            } else {
                (
                    destination_y as i32 - y_offset,
                    height_i32 - 1 - x as i32 + x_offset,
                )
            };
            let output_start = output_row + x * channels;
            if source_x >= 0 && source_x < width_i32 && source_y >= 0 && source_y < height_i32 {
                let source_start = (source_y as usize * width + source_x as usize) * channels;
                output[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    output[output_start + channel] =
                        super::rotate_fill_sample(fill, channels, channel);
                }
            }
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    (vector_blocks, scalar_tail)
}

#[inline(always)]
pub(super) fn finish_native_rotation(
    img: &DynamicImage,
    width: u32,
    height: u32,
    output: Vec<u8>,
    channels: usize,
    vector_blocks: u64,
    scalar_tail: u64,
    path: &'static str,
) -> Result<Option<DynamicImage>, PilError> {
    crate::compute::record_pipeline_operation_path(path);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

pub(crate) fn simd_rotate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        center,
        translate,
        filter,
        nearest,
        ..
    } = op
    else {
        return Err(PilError::ValueError("expected Rotate op".into()));
    };
    // Indexed Pillow images are defined to rotate with nearest-neighbor
    // sampling. The operation flag records only the explicit resample
    // request, so include the logical mode before choosing the data kernel.
    let nearest = *nearest || matches!(mode, Some("1" | "P"));
    if let Some(result) =
        simd_empty_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    if rotate_identity_contract(*angle, *center, *translate) {
        if let Some(result) = native_identity_rotate(img, mode)? {
            return Ok(preserve_mode(img, result));
        }
    }
    if rotate_uses_discrete_fast_path(
        *angle,
        *center,
        *translate,
        *expand,
        img.width(),
        img.height(),
    ) && let Some(result) =
        simd_right_angle_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    if nearest
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) =
            simd_nearest_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    if !nearest
        && matches!(filter, ResampleFilter::Bilinear)
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) = simd_typed_bilinear_rotate_native(
            img, *angle, *expand, *fill, *center, *translate, mode,
        )?
    {
        return Ok(result);
    }
    if !nearest
        && matches!(filter, ResampleFilter::Bilinear)
        && !rotate_uses_discrete_fast_path(
            *angle,
            *center,
            *translate,
            *expand,
            img.width(),
            img.height(),
        )
        && let Some(result) =
            simd_bilinear_rotate_native(img, *angle, *expand, *fill, *center, *translate, mode)?
    {
        return Ok(result);
    }
    Err(simd_unsupported("Rotate"))
}
