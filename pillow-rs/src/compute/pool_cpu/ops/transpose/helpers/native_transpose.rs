//! Wide-lane tiled transpose for opaque native pixel bytes.
//!
//! This helper preserves the native byte transpose paths for both axis-swapping
//! and same-dimension methods. Parallel collection remains owned by
//! `pool_parallel`.

use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::{EncodableLayout, GenericImageView, ImageBuffer, Pixel};
use wide::{f32x4, u8x16, u16x8};

/// Transposes a typed byte image through the mode's selected native strategy.
pub(crate) fn transpose_u8<P, const CHANNELS: usize, const TILE: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    method: &TransposeMethod,
) -> Result<(ImageBuffer<P, Vec<u8>>, u64, u64), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (width, height) = image.dimensions();
    let (bytes, output_width, output_height, vector_blocks, scalar_tail) =
        transpose_bytes::<CHANNELS, TILE>(image.as_raw(), width, height, method)?;
    let output = ImageBuffer::from_raw(output_width, output_height, bytes)
        .ok_or_else(|| PilError::InternalError("native byte transpose shape mismatch".into()))?;
    Ok((output, vector_blocks, scalar_tail))
}

/// Transposes typed u16 samples as opaque native-endian byte pairs.
pub(crate) fn transpose_u16<P, const TILE: usize>(
    image: &ImageBuffer<P, Vec<u16>>,
    method: &TransposeMethod,
) -> Result<(ImageBuffer<P, Vec<u16>>, u64, u64), PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let (width, height) = image.dimensions();
    let source = image.as_raw().as_slice().as_bytes();
    let (bytes, output_width, output_height, vector_blocks, scalar_tail) =
        transpose_bytes::<2, TILE>(source, width, height, method)?;
    let mut chunks = bytes.chunks_exact(2);
    let output_samples = chunks
        .by_ref()
        .map(|sample| u16::from_ne_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    if !chunks.remainder().is_empty() {
        return Err(PilError::InternalError("native u16 transpose returned a partial sample".into()));
    }
    let output = ImageBuffer::from_raw(output_width, output_height, output_samples)
        .ok_or_else(|| PilError::InternalError("native u16 transpose shape mismatch".into()))?;
    Ok((output, vector_blocks, scalar_tail))
}

fn transpose_bytes<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    width: u32,
    height: u32,
    method: &TransposeMethod,
) -> Result<(Vec<u8>, u32, u32, u64, u64), PilError> {
    let input = crate::checked_dims::CheckedDims::new(width, height, CHANNELS as u8)?;
    if source.len() != input.total_bytes() {
        return Err(PilError::InternalError("native transpose source shape mismatch".into()));
    }
    let swaps_axes = matches!(
        method,
        TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse
    );
    let (output_width, output_height) = if swaps_axes {
        (height, width)
    } else {
        (width, height)
    };
    let output_dims = crate::checked_dims::CheckedDims::new(
        output_width,
        output_height,
        CHANNELS as u8,
    )?;
    let mut output = output_dims.alloc_buffer();
    let width = width as usize;
    let height = height as usize;
    let (vector_blocks, scalar_tail) = match method {
        TransposeMethod::FlipLeftRight
        | TransposeMethod::FlipTopBottom
        | TransposeMethod::Rotate180 => {
            transpose_same_dimensions::<CHANNELS>(source, &mut output, width, height, method)?
        }
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => {
            if width >= TILE && height >= TILE {
                transpose_tiled::<CHANNELS, TILE>(source, &mut output, width, height, method);
                let tiles = (width / TILE) * (height / TILE);
                (
                    (tiles * TILE) as u64,
                    ((width * height - tiles * TILE * TILE) * CHANNELS) as u64,
                )
            } else {
                transpose_gathered_axis_swap::<CHANNELS>(
                    source,
                    &mut output,
                    width,
                    height,
                    method,
                )?
            }
        }
    };
    Ok((output, output_width, output_height, vector_blocks, scalar_tail))
}

fn transpose_tiled<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    method: &TransposeMethod,
) {
    let (reverse_x, reverse_y) = match method {
        TransposeMethod::Rotate90 => (true, false),
        TransposeMethod::Rotate270 => (false, true),
        TransposeMethod::Transpose => (false, false),
        TransposeMethod::Transverse => (true, true),
        _ => unreachable!("same-dimension transpose is handled separately"),
    };
    let rows_per_group = width.min(32);
    let output_stride = height * CHANNELS;
    let group_stride = output_stride * rows_per_group;
    for (group, rows) in output.chunks_mut(group_stride).enumerate() {
        transpose_row_group::<CHANNELS, TILE>(
            source,
            rows,
            width,
            height,
            group * rows_per_group,
            reverse_x,
            reverse_y,
        );
    }
}

fn transpose_same_dimensions<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    method: &TransposeMethod,
) -> Result<(u64, u64), PilError> {
    let row_bytes = width.checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("native transpose row size overflow".into()))?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height {
        let (source_y, mirror) = match method {
            TransposeMethod::FlipLeftRight => (y, true),
            TransposeMethod::FlipTopBottom => (height - 1 - y, false),
            TransposeMethod::Rotate180 => (height - 1 - y, true),
            _ => return Err(PilError::InternalError(
                "native same-dimension transpose selected for an axis swap".into(),
            )),
        };
        let source_start = source_y * row_bytes;
        let output_start = y * row_bytes;
        let source_row = &source[source_start..source_start + row_bytes];
        let output_row = &mut output[output_start..output_start + row_bytes];
        let (blocks, tail) = if mirror {
            crate::compute::pool_cpu::ops::transpose::helpers::native_mirror::mirror_native_row(
                source_row,
                output_row,
                width,
                CHANNELS,
            )
        } else {
            copy_native_bytes(source_row, output_row)
        }
        .ok_or_else(|| PilError::InternalError("native same-dimension transpose shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

fn transpose_gathered_axis_swap<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    method: &TransposeMethod,
) -> Result<(u64, u64), PilError> {
    let source_row_bytes = width * CHANNELS;
    let output_row_bytes = height * CHANNELS;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_y in 0..width {
        let (source_x, reverse_source_y) = gathered_source_column(method, width, output_y);
        let output_start = output_y * output_row_bytes;
        let (blocks, tail) = transpose_gathered_row(
            source,
            &mut output[output_start..output_start + output_row_bytes],
            source_row_bytes,
            height,
            CHANNELS,
            source_x,
            reverse_source_y,
        )
        .ok_or_else(|| PilError::InternalError("native gathered transpose shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    Ok((vector_blocks, scalar_tail))
}

fn transpose_gathered_row(
    source: &[u8],
    output: &mut [u8],
    source_row_bytes: usize,
    source_height: usize,
    channels: usize,
    source_x: usize,
    reverse_source_y: bool,
) -> Option<(u64, u64)> {
    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let mut block = [0u8; 16];
        for (lane, value) in block.iter_mut().enumerate() {
            let output_byte = output_offset + lane;
            let output_pixel = output_byte / channels;
            let channel = output_byte % channels;
            let source_y = if reverse_source_y {
                source_height - 1 - output_pixel
            } else {
                output_pixel
            };
            let source_offset = source_y
                .checked_mul(source_row_bytes)?
                .checked_add(source_x.checked_mul(channels)?)?
                .checked_add(channel)?;
            *value = *source.get(source_offset)?;
        }
        output[output_offset..output_offset + 16].copy_from_slice(&u8x16::new(block).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output.len() - vector_len;
    for output_offset in vector_len..output.len() {
        let output_pixel = output_offset / channels;
        let channel = output_offset % channels;
        let source_y = if reverse_source_y {
            source_height - 1 - output_pixel
        } else {
            output_pixel
        };
        let source_offset = source_y
            .checked_mul(source_row_bytes)?
            .checked_add(source_x.checked_mul(channels)?)?
            .checked_add(channel)?;
        output[output_offset] = *source.get(source_offset)?;
    }
    Some((vector_blocks, scalar_tail as u64))
}

#[inline]
fn gathered_source_column(
    method: &TransposeMethod,
    width: usize,
    output_row: usize,
) -> (usize, bool) {
    match method {
        TransposeMethod::Rotate90 => (width - 1 - output_row, false),
        TransposeMethod::Rotate270 | TransposeMethod::Transpose => {
            (output_row, matches!(method, TransposeMethod::Rotate270))
        }
        TransposeMethod::Transverse => (width - 1 - output_row, true),
        _ => unreachable!("same-dimension transpose is handled separately"),
    }
}

fn copy_native_bytes(source: &[u8], output: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != output.len() {
        return None;
    }
    if source.is_empty() {
        return Some((0, 0));
    }
    let vector_len = source.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for offset in (0..vector_len).step_by(16) {
        let block = u8x16::new(<[u8; 16]>::try_from(&source[offset..offset + 16]).ok()?);
        output[offset..offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = source.len() - vector_len;
    if scalar_tail != 0 {
        let mut padded = [0u8; 16];
        padded[..scalar_tail].copy_from_slice(&source[vector_len..]);
        let block = u8x16::new(padded);
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

#[inline]
fn transpose_tile<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    source_stride: usize,
    source_x: usize,
    source_y: usize,
) -> [u8x16; TILE] {
    let mut rows = [u8x16::splat(0); TILE];
    for (row_index, row) in rows.iter_mut().enumerate() {
        let start = (source_y + row_index) * source_stride + source_x * CHANNELS;
        let mut bytes = [0u8; 16];
        bytes[..TILE * CHANNELS].copy_from_slice(&source[start..start + TILE * CHANNELS]);
        *row = u8x16::new(bytes);
        if CHANNELS == 3 {
            *row = row.swizzle_relaxed(u8x16::new([
                0, 1, 2, 0, 3, 4, 5, 0, 6, 7, 8, 0, 9, 10, 11, 0,
            ]));
        }
    }
    if CHANNELS == 1 {
        let low = u16x8::transpose(std::array::from_fn(|index| {
            bytemuck::cast(u8x16::unpack_low(rows[index * 2], rows[index * 2 + 1]))
        }));
        let high = u16x8::transpose(std::array::from_fn(|index| {
            bytemuck::cast(u8x16::unpack_high(rows[index * 2], rows[index * 2 + 1]))
        }));
        for (output, column) in rows.iter_mut().zip(low.into_iter().chain(high)) {
            *output = bytemuck::cast(column);
        }
    } else if CHANNELS == 2 {
        let columns = u16x8::transpose(std::array::from_fn(|index| bytemuck::cast(rows[index])));
        for (output, column) in rows.iter_mut().zip(columns) {
            *output = bytemuck::cast(column);
        }
    } else {
        let columns = f32x4::transpose(std::array::from_fn(|index| bytemuck::cast(rows[index])));
        for (output, column) in rows.iter_mut().zip(columns) {
            *output = bytemuck::cast(column);
        }
    }
    rows
}

fn transpose_row_group<const CHANNELS: usize, const TILE: usize>(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    output_y_start: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;
    let row_count = output.len() / output_stride;
    let full_rows = row_count / TILE * TILE;
    let full_columns = height / TILE * TILE;
    let store_mask = u8x16::new(std::array::from_fn(|byte| {
        if CHANNELS == 3 {
            let pixel = byte / 3;
            if pixel < TILE {
                let pixel = if reverse_y { TILE - 1 - pixel } else { pixel };
                (pixel * 4 + byte % 3) as u8
            } else {
                0
            }
        } else if reverse_y {
            ((TILE - 1 - byte / CHANNELS) * CHANNELS + byte % CHANNELS) as u8
        } else {
            byte as u8
        }
    }));
    for block_x in (0..full_columns).step_by(32) {
        for local_y in (0..full_rows).step_by(TILE) {
            let output_y = output_y_start + local_y;
            let source_x = if reverse_x { width - output_y - TILE } else { output_y };
            for output_x in (block_x..(block_x + 32).min(full_columns)).step_by(TILE) {
                let source_y = if reverse_y { height - output_x - TILE } else { output_x };
                let columns = transpose_tile::<CHANNELS, TILE>(source, source_stride, source_x, source_y);
                for (column_index, column) in columns.into_iter().enumerate() {
                    let row = if reverse_x { TILE - 1 - column_index } else { column_index };
                    let start = (local_y + row) * output_stride + output_x * CHANNELS;
                    let column = if CHANNELS == 3 || reverse_y {
                        column.swizzle_relaxed(store_mask)
                    } else {
                        column
                    };
                    output[start..start + TILE * CHANNELS]
                        .copy_from_slice(&column.to_array()[..TILE * CHANNELS]);
                }
            }
        }
    }
    for (local_y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let output_y = output_y_start + local_y;
        let source_x = if reverse_x { width - 1 - output_y } else { output_y };
        let first_x = if local_y < full_rows { full_columns } else { 0 };
        for output_x in first_x..height {
            let source_y = if reverse_y { height - 1 - output_x } else { output_x };
            let source_start = source_y * source_stride + source_x * CHANNELS;
            let output_start = output_x * CHANNELS;
            row[output_start..output_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }
}
