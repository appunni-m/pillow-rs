//! Native-byte nearest-neighbor SIMD kernels for Resize and Thumbnail.

use super::super::{SIMD_RESIZE_NEAREST_BYTES, simd_unsupported};
use super::{boxed_nearest_indices, resize_nearest_indices};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::DynamicImage;
use wide::u8x16;

/// Gather one nearest-neighbour output block when the selected source pixels
/// do not fit in a single sixteen-byte shuffle window. Byte gathers are not
/// portable across the SIMD targets supported by this crate, so the index
/// arithmetic and loads stay scalar; packing and the contiguous output store
/// remain part of the SIMD data plane.
fn nearest_gather_block(
    source: &[u8],
    source_row: usize,
    x_indices: &[usize],
    output_x: usize,
    pixel_count: usize,
    channels: usize,
) -> Option<[u8; SIMD_RESIZE_NEAREST_BYTES]> {
    let mut block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
    for pixel in 0..pixel_count {
        let source_pixel = x_indices
            .get(output_x.checked_add(pixel)?)?
            .checked_mul(channels)?;
        let source_start = source_row.checked_add(source_pixel)?;
        let source_end = source_start.checked_add(channels)?;
        let output_start = pixel.checked_mul(channels)?;
        block
            .get_mut(output_start..output_start.checked_add(channels)?)?
            .copy_from_slice(source.get(source_start..source_end)?);
    }
    Some(block)
}

pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_nearest(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width_usize =
        usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height_usize =
        usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let x_indices = resize_nearest_indices(img.width(), output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let y_indices = resize_nearest_indices(img.height(), output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let output_len = output_width_usize
        .checked_mul(output_height_usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD resize source buffer shape mismatch".into(),
        ));
    }
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let vector_width = output_width_usize / pixels_per_vector * pixels_per_vector;
    let block_bytes = pixels_per_vector * channels;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output_y
            .checked_mul(output_width_usize)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        if output_width_usize < pixels_per_vector {
            // A short row still has a vector data plane: gather the selected
            // samples into a padded u8x16 block and store only the valid
            // prefix. This keeps narrow ImageOps.pad/resize rows out of the
            // scalar-only admission bucket without reading past the source.
            let source_start = x_indices[0]
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let source_end = x_indices[output_width_usize - 1]
                .checked_mul(channels)
                .and_then(|offset| offset.checked_add(channels))
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let span = source_end
                .checked_sub(source_start)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let values = if span <= SIMD_RESIZE_NEAREST_BYTES {
                let mut source_block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                let source_slice_start = source_row
                    .checked_add(source_start)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let source_slice_end = source_slice_start
                    .checked_add(span)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                source_block[..span].copy_from_slice(
                    source
                        .get(source_slice_start..source_slice_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
                let mut indices = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                for pixel in 0..output_width_usize {
                    let source_pixel = x_indices[pixel]
                        .checked_mul(channels)
                        .and_then(|offset| offset.checked_sub(source_start))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    for channel in 0..channels {
                        indices[pixel * channels + channel] = u8::try_from(source_pixel + channel)
                            .map_err(|_| simd_unsupported("Resize"))?;
                    }
                }
                u8x16::new(source_block)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            } else {
                u8x16::new(
                    nearest_gather_block(
                        source,
                        source_row,
                        &x_indices,
                        0,
                        output_width_usize,
                        channels,
                    )
                    .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .to_array()
            };
            let output_bytes = output_width_usize
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_row..output_row + output_bytes)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(&values[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        for output_x in (0..vector_width).step_by(pixels_per_vector) {
            let source_start = x_indices[output_x]
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let source_end = x_indices[output_x + pixels_per_vector - 1]
                .checked_mul(channels)
                .and_then(|offset| offset.checked_add(channels))
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let span = source_end
                .checked_sub(source_start)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let values = if span <= SIMD_RESIZE_NEAREST_BYTES {
                let mut source_block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                let source_slice_start = source_row
                    .checked_add(source_start)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let source_slice_end = source_slice_start
                    .checked_add(span)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                source_block[..span].copy_from_slice(
                    source
                        .get(source_slice_start..source_slice_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
                let mut indices = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                for pixel in 0..pixels_per_vector {
                    let source_pixel = x_indices[output_x + pixel]
                        .checked_mul(channels)
                        .and_then(|offset| offset.checked_sub(source_start))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    for channel in 0..channels {
                        indices[pixel * channels + channel] = u8::try_from(source_pixel + channel)
                            .map_err(|_| simd_unsupported("Resize"))?;
                    }
                }
                u8x16::new(source_block)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            } else {
                u8x16::new(
                    nearest_gather_block(
                        source,
                        source_row,
                        &x_indices,
                        output_x,
                        pixels_per_vector,
                        channels,
                    )
                    .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .to_array()
            };
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_start..output_start + block_bytes)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(&values[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        for output_x in vector_width..output_width_usize {
            let source_start = source_row
                .checked_add(
                    x_indices[output_x]
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_start..output_start + channels)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(
                    source
                        .get(source_start..source_start + channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Execute boxed nearest-neighbour sampling for native byte layouts. The
/// coordinate equations and output packing use SIMD lanes; only the portable
/// byte gathers and unavoidable short-row/tail stores are scalar.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_resize_nearest_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Fit"))?;
    let x_indices = boxed_nearest_indices(
        source_width as u32,
        output_width as u32,
        box_left,
        box_right,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    let y_indices = boxed_nearest_indices(
        source_height as u32,
        output_height as u32,
        box_top,
        box_bottom,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    let output_len = output_width
        .checked_mul(output_height)
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
    let source = img.as_bytes();
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let vector_width = output_width / pixels_per_vector * pixels_per_vector;
    let block_bytes = pixels_per_vector * channels;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Fit"))?;
        if output_width < pixels_per_vector {
            let values = u8x16::new(
                nearest_gather_block(source, source_row, &x_indices, 0, output_width, channels)
                    .ok_or_else(|| simd_unsupported("Fit"))?,
            )
            .to_array();
            let output_bytes = output_width
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_row..output_row + output_bytes)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(&values[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        for output_x in (0..vector_width).step_by(pixels_per_vector) {
            let values = u8x16::new(
                nearest_gather_block(
                    source,
                    source_row,
                    &x_indices,
                    output_x,
                    pixels_per_vector,
                    channels,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?,
            )
            .to_array();
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_start..output_start + block_bytes)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(&values[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        for output_x in vector_width..output_width {
            let source_start = source_row
                .checked_add(
                    x_indices[output_x]
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_start..output_start + channels)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(
                    source
                        .get(source_start..source_start + channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                );
            scalar_tail = scalar_tail.saturating_add(1);
        }
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
