//! CPU transpose family with portable, native-byte, and architecture paths.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image_utils::raw_bytes_to_image;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

#[path = "native_dispatch.rs"]
mod native_transpose;
#[cfg(target_arch = "x86_64")]
#[path = "arch.rs"]
mod transpose_arch;
#[cfg(target_arch = "x86_64")]
#[path = "rgba_x86_64.rs"]
mod transpose_rgba_x86_64;

/// Execute a Transpose operation.
pub(in crate::compute::pool_cpu::ops::transpose) const TRANSPOSE_TILE_SIZE: u32 = 32;

const TRANSPOSE_RGBA_TILE_SIZE: u32 = 16;
const TRANSPOSE_TILE_THRESHOLD_PIXELS: usize = 256 * 1024;

#[inline]
pub(super) fn should_tile_transpose(width: u32, height: u32) -> bool {
    width >= TRANSPOSE_TILE_SIZE
        && height >= TRANSPOSE_TILE_SIZE
        && (width as usize).saturating_mul(height as usize) >= TRANSPOSE_TILE_THRESHOLD_PIXELS
}

/// Transpose a large byte image in bounded output-row tiles.
///
/// Each task owns complete output rows and visits bounded rectangles within
/// them. Writing each row segment contiguously keeps destination indexing out
/// of the pixel loop while the source gathers stay within one small rectangle.
/// The small-image path deliberately keeps its old order.
pub(in crate::compute::pool_cpu::ops::transpose) fn transpose_bytes_tiled<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
    tile_size: u32,
) {
    let width = width as usize;
    let height = height as usize;
    let tile_size = tile_size as usize;
    let output_stride = height * CHANNELS;
    let source_stride = width * CHANNELS;

    let tile_rows = width.div_ceil(tile_size);
    let (reverse_x, reverse_y) = match method {
        TransposeMethod::Transpose => (false, false),
        TransposeMethod::Transverse => (true, true),
        TransposeMethod::Rotate90 => (true, false),
        TransposeMethod::Rotate270 => (false, true),
        _ => unreachable!("unsupported tiled transpose method"),
    };
    let process_tile = |tile_index: usize, rows: &mut [u8]| {
        for block_x in (0..height).step_by(tile_size) {
            let end_x = (block_x + tile_size).min(height);
            for (local_y, row) in rows.chunks_exact_mut(output_stride).enumerate() {
                let output_y = tile_index * tile_size + local_y;
                let source_x = if reverse_x {
                    width - 1 - output_y
                } else {
                    output_y
                };
                let pixels = row[block_x * CHANNELS..end_x * CHANNELS].chunks_exact_mut(CHANNELS);
                // Each segment contains complete native samples. Its source
                // indices advance by one row, bounded by this rectangle even
                // when the final tile is partial or the orientation reverses.
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
    };

    for tile_index in 0..tile_rows {
        let output_y_start = tile_index * tile_size;
        let output_y_end = (output_y_start + tile_size).min(width);
        let row_start = output_y_start * output_stride;
        let row_end = output_y_end * output_stride;
        let rows = &mut output[row_start..row_end];
        process_tile(tile_index, rows);
    }
}

/// Transpose small native byte images with complete row and pixel slices.
/// This avoids repeated coordinate validation and saturating arithmetic in
/// generic pixel accessors. Trim trailing storage before reversing source
/// rows: ImageBuffer permits extra samples after the logical raster.
pub(super) fn transpose_bytes_serial<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
) {
    let source = &source[..output.len()];
    let source_stride = width as usize * CHANNELS;
    let output_stride = height as usize * CHANNELS;
    let reverse_x = matches!(
        method,
        TransposeMethod::Rotate90 | TransposeMethod::Transverse
    );
    let reverse_y = matches!(
        method,
        TransposeMethod::Rotate270 | TransposeMethod::Transverse
    );
    for (y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let source_x = if reverse_x { width as usize - 1 - y } else { y };
        let offset = source_x * CHANNELS;
        let source_rows = source.chunks_exact(source_stride);
        if reverse_y {
            for (pixel, original) in row.chunks_exact_mut(CHANNELS).zip(source_rows.rev()) {
                pixel.copy_from_slice(&original[offset..offset + CHANNELS]);
            }
        } else {
            for (pixel, original) in row.chunks_exact_mut(CHANNELS).zip(source_rows) {
                pixel.copy_from_slice(&original[offset..offset + CHANNELS]);
            }
        }
    }
}

/// Copy complete native rows for flips and half turns. Pixel chunks keep
/// channels (including opaque packed I/F words) together without repeated
/// coordinate validation or a per-pixel image accessor.
pub(super) fn transpose_bytes_rows<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    dimensions: &CheckedDims,
    reverse_x: bool,
    reverse_y: bool,
) {
    let height = dimensions.height as usize;
    let stride = dimensions.row_stride();
    let copy_rows = |first_y: usize, rows: &mut [u8]| {
        for (local_y, row) in rows.chunks_exact_mut(stride).enumerate() {
            let y = first_y + local_y;
            let source_y = if reverse_y { height - 1 - y } else { y };
            let input = &source[source_y * stride..(source_y + 1) * stride];
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
    };

    copy_rows(0, output);
}

pub(in crate::compute::pool_cpu::ops::transpose) fn execute_transpose(
    img: &DynamicImage,
    method: &TransposeMethod,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    if let Some(output) = native_transpose::execute_flip(img, method, channels)? {
        return Ok(output);
    }
    if let Some(output) = native_transpose::execute_axis_swap(img, method, channels)? {
        return Ok(output);
    }
    match method {
        TransposeMethod::FlipLeftRight => Ok(img.fliph()),
        TransposeMethod::FlipTopBottom => Ok(img.flipv()),
        // PIL rotates counter-clockwise; image crate rotates clockwise.
        // PIL ROTATE_90 (CCW) = image crate rotate270 (CW)
        // PIL ROTATE_270 (CCW) = image crate rotate90 (CW)
        TransposeMethod::Rotate90 => Ok(img.rotate270()),
        TransposeMethod::Rotate180 => Ok(img.rotate180()),
        TransposeMethod::Rotate270 => Ok(img.rotate90()),
        TransposeMethod::Transpose | TransposeMethod::Transverse => {
            Ok(img.transpose_diagonal(matches!(method, TransposeMethod::Transverse)))
        }
    }
}
