//! Native pixel movement shared by the transpose family.

use crate::raster::{GenericImageView, ImageBuffer, Pixel};
use crate::pipeline::TransposeMethod;

/// An identity transpose run may defer its required owned output until the
/// batch terminal only when the input contains exactly its logical pixels.
/// Oversized raw carriers must pass through a real transpose so trailing
/// samples are discarded with the same normalization as a single operation.
pub(crate) fn identity_can_defer_output(image: &crate::raster::DynamicImage) -> bool {
    crate::checked_dims::CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        image.color().bytes_per_pixel(),
    )
    .is_ok_and(|dims| dims.total_bytes() == image.as_bytes().len())
}

const TRANSPOSE_TILE_SIZE: usize = 32;
const TRANSPOSE_RGBA_TILE_SIZE: usize = 16;
const TRANSPOSE_TILE_THRESHOLD_PIXELS: usize = 256 * 1024;

/// Reverses complete source rows using the same row-span algorithm as the
/// mode-preserving `DynamicImage::flipv` fallback.
pub(crate) fn flip_vertical_samples<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
    P::Subpixel: Copy,
{
    let (width, height) = image.dimensions();
    let source = image.as_raw();
    if height == 0 {
        return image.clone();
    }
    let row_len = source.len() / height as usize;
    if row_len == 0 {
        return image.clone();
    }

    let mut output = Vec::with_capacity(source.len());
    for row in source.chunks_exact(row_len).rev() {
        output.extend_from_slice(row);
    }
    ImageBuffer::from_raw(width, height, output)
        .expect("vertical flip preserves the source sample count")
}

/// Reverses pixel order through the same typed pixel lookup as the original
/// generic horizontal image transform.
pub(crate) fn flip_horizontal_samples<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel(width.saturating_sub(1).saturating_sub(x), y)
    })
}

/// Reverses tightly packed byte rows when their exact pixel shape is valid.
/// `None` preserves the old generic fallback for malformed or oversized data.
pub(crate) fn flip_vertical_bytes<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    let row_bytes = usize::try_from(width).ok()?.checked_mul(CHANNELS)?;
    let total_bytes = row_bytes.checked_mul(usize::try_from(height).ok()?)?;
    if source.len() != total_bytes {
        return None;
    }
    if height == 0 || row_bytes == 0 {
        return Some(source.to_vec());
    }

    let mut output = Vec::with_capacity(total_bytes);
    for output_y in 0..height as usize {
        let source_y = height as usize - 1 - output_y;
        let start = source_y.checked_mul(row_bytes)?;
        output.extend_from_slice(source.get(start..start.checked_add(row_bytes)?)?);
    }
    Some(output)
}

/// Mirrors complete native byte pixels, matching the old tight-row fast path.
pub(crate) fn flip_horizontal_bytes<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }
    let dimensions = crate::checked_dims::CheckedDims::new(width, height, CHANNELS as u8).ok()?;
    let row_len = dimensions.row_stride();
    let source = source.get(..dimensions.total_bytes())?;
    let mut output = dimensions.alloc_buffer();
    for (source_row, output_row) in source
        .chunks_exact(row_len)
        .zip(output.chunks_exact_mut(row_len))
    {
        for (destination, source_pixel) in output_row
            .chunks_exact_mut(CHANNELS)
            .zip(source_row.rchunks_exact(CHANNELS))
        {
            destination.copy_from_slice(source_pixel);
        }
    }
    Some(output)
}

/// Applies the original native-byte CPU transpose path when dimensions and
/// accessible source storage satisfy its checked-layout requirements.
pub(crate) fn transpose_bytes<const CHANNELS: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
) -> Result<Option<(u32, u32, Vec<u8>)>, crate::error::PilError> {
    let swaps_axes = matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    );
    if width == 0 || height == 0 {
        return Ok(None);
    }
    if swaps_axes && width == 1 && height == 1 {
        return Ok(None);
    }
    let input = crate::checked_dims::CheckedDims::new(width, height, CHANNELS as u8)?;
    let Some(source) = source.get(..input.total_bytes()) else {
        return Ok(None);
    };
    let (output_width, output_height) = if swaps_axes { (height, width) } else { (width, height) };
    let output_dims = crate::checked_dims::CheckedDims::new(
        output_width,
        output_height,
        CHANNELS as u8,
    )?;
    let mut output = output_dims.alloc_buffer();

    if swaps_axes {
        let tile_size = if CHANNELS == 4 {
            TRANSPOSE_RGBA_TILE_SIZE
        } else {
            TRANSPOSE_TILE_SIZE
        };
        if should_tile_transpose(width, height) {
            transpose_bytes_tiled(source, &mut output, width, height, method, tile_size);
        } else {
            transpose_bytes_serial(source, &mut output, width, height, method);
        }
    } else {
        let (reverse_x, reverse_y) = match method {
            TransposeMethod::FlipLeftRight => (true, false),
            TransposeMethod::FlipTopBottom => (false, true),
            TransposeMethod::Rotate180 => (true, true),
            _ => return Ok(None),
        };
        transpose_bytes_rows::<CHANNELS>(source, &mut output, &input, reverse_x, reverse_y);
    }

    Ok(Some((output_width, output_height, output)))
}

pub(crate) fn should_tile_transpose(width: u32, height: u32) -> bool {
    width >= TRANSPOSE_TILE_SIZE as u32
        && height >= TRANSPOSE_TILE_SIZE as u32
        && (width as usize).saturating_mul(height as usize) >= TRANSPOSE_TILE_THRESHOLD_PIXELS
}

fn transpose_bytes_rows<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    dimensions: &crate::checked_dims::CheckedDims,
    reverse_x: bool,
    reverse_y: bool,
) {
    let height = dimensions.height as usize;
    let stride = dimensions.row_stride();
    for y in 0..height {
        let source_y = if reverse_y { height - 1 - y } else { y };
        let input = &source[source_y * stride..(source_y + 1) * stride];
        let row = &mut output[y * stride..(y + 1) * stride];
        if reverse_x {
            for (pixel, original) in row
                .chunks_exact_mut(CHANNELS)
                .zip(input.chunks_exact(CHANNELS).rev())
            {
                pixel.copy_from_slice(original);
            }
        } else {
            row.copy_from_slice(input);
        }
    }
}

fn transpose_bytes_serial<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
) {
    let width = width as usize;
    let height = height as usize;
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;
    let reverse_x = matches!(method, TransposeMethod::Rotate90 | TransposeMethod::Transverse);
    let reverse_y = matches!(method, TransposeMethod::Rotate270 | TransposeMethod::Transverse);
    for (output_y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let source_x = if reverse_x { width - 1 - output_y } else { output_y };
        let offset = source_x * CHANNELS;
        let source_rows = source.chunks_exact(source_stride);
        if reverse_y {
            for (pixel, original) in row
                .chunks_exact_mut(CHANNELS)
                .zip(source_rows.rev())
            {
                let start = offset;
                pixel.copy_from_slice(&original[start..start + CHANNELS]);
            }
        } else {
            for (pixel, original) in row.chunks_exact_mut(CHANNELS).zip(source_rows) {
                pixel.copy_from_slice(&original[offset..offset + CHANNELS]);
            }
        }
    }
}

fn transpose_bytes_tiled<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
    tile_size: usize,
) {
    let width = width as usize;
    let height = height as usize;
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;
    let reverse_x = matches!(method, TransposeMethod::Rotate90 | TransposeMethod::Transverse);
    let reverse_y = matches!(method, TransposeMethod::Rotate270 | TransposeMethod::Transverse);

    for tile_y in (0..width).step_by(tile_size) {
        let tile_y_end = (tile_y + tile_size).min(width);
        for block_x in (0..height).step_by(tile_size) {
            let block_x_end = (block_x + tile_size).min(height);
            for output_y in tile_y..tile_y_end {
                let source_x = if reverse_x { width - 1 - output_y } else { output_y };
                let row_start = output_y * output_stride;
                let row = &mut output[row_start..row_start + output_stride];
                let pixels = row[block_x * CHANNELS..block_x_end * CHANNELS]
                    .chunks_exact_mut(CHANNELS);
                if reverse_y {
                    let first = (height - 1 - block_x) * source_stride + source_x * CHANNELS;
                    for (offset, pixel) in pixels.enumerate() {
                        let source_index = first - offset * source_stride;
                        pixel.copy_from_slice(&source[source_index..source_index + CHANNELS]);
                    }
                } else {
                    let first = block_x * source_stride + source_x * CHANNELS;
                    for (offset, pixel) in pixels.enumerate() {
                        let source_index = first + offset * source_stride;
                        pixel.copy_from_slice(&source[source_index..source_index + CHANNELS]);
                    }
                }
            }
        }
    }
}

/// Runs the original typed image transforms without consulting a mode tag.
pub(crate) fn transpose_samples<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    method: &TransposeMethod,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    if matches!(method, TransposeMethod::FlipLeftRight) {
        return flip_horizontal_samples(image);
    }
    if matches!(method, TransposeMethod::FlipTopBottom) {
        return flip_vertical_samples(image);
    }
    let swaps_axes = matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    );
    let (output_width, output_height) = if swaps_axes { (height, width) } else { (width, height) };
    ImageBuffer::from_fn(output_width, output_height, |x, y| {
        let (source_x, source_y) = transpose_source_coordinates(method, width, height, x, y);
        *image.get_pixel(source_x, source_y)
    })
}

fn transpose_source_coordinates(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width.saturating_sub(1).saturating_sub(x), y),
        TransposeMethod::FlipTopBottom => (x, height.saturating_sub(1).saturating_sub(y)),
        TransposeMethod::Rotate90 => (width.saturating_sub(1).saturating_sub(y), x),
        TransposeMethod::Rotate180 => (
            width.saturating_sub(1).saturating_sub(x),
            height.saturating_sub(1).saturating_sub(y),
        ),
        TransposeMethod::Rotate270 => (y, height.saturating_sub(1).saturating_sub(x)),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (
            width.saturating_sub(1).saturating_sub(y),
            height.saturating_sub(1).saturating_sub(x),
        ),
    }
}
