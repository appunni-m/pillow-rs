//! Pillow-compatible byte-oriented I;16 Offset kernels.

use super::byte::{circular_byte_block, narrow_rotated_row};
use crate::error::PilError;
use crate::raster::buffer::ImageBuffer;
use crate::raster::color::Luma;
use wide::u8x16;

/// Copy one Pillow `I;16*` offset row through native byte vectors.
///
/// The scalar reference follows Pillow's historical `ImageChops` byte path:
/// it rotates the first `width` bytes of a `width * 2` row and leaves the
/// remaining bytes zero. Preserve that observable contract instead of
/// treating the samples as ordinary `u16` pixels. The byte-order conversion
/// is control-plane work; the copied portion uses the same circular `u8x16`
/// kernel as ordinary byte images.
#[inline]
fn offset_luma16_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    shift_bytes: usize,
) -> Option<(u64, u64)> {
    let row_bytes = width.checked_mul(2)?;
    if width < 16 || source.len() != row_bytes || output.len() != row_bytes || shift_bytes >= width
    {
        return None;
    }
    let visible_source = source.get(..width)?;
    let vector_len = width / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let source_start = (shift_bytes + output_offset) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = width - vector_len;
    if scalar_tail != 0 {
        let source_start = (shift_bytes + vector_len) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[vector_len..width].copy_from_slice(&block.to_array()[..scalar_tail]);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Build one masked vector for a narrow `I;16*` offset frame.
///
/// The public `ImageChops.offset` contract only copies the first `width`
/// bytes of each `width * 2` storage row. A vector block may therefore cross
/// both a row's copied/zero boundary and the next row's boundary. Scalar
/// arithmetic computes those boundaries; the selected source bytes and the
/// final masked store remain in `u8x16`.
#[inline]
fn offset_luma16_narrow_block(
    source: &[u8],
    output_offset: usize,
    active_len: usize,
    width: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<u8x16> {
    let storage_row_len = width.checked_mul(2)?;
    if active_len == 0
        || active_len > 16
        || output_offset.checked_add(active_len)? > source.len()
        || width == 0
        || width >= 16
        || height == 0
        || yshift >= height
        || shift_bytes >= width
    {
        return None;
    }

    let mut block = u8x16::splat(0);
    let mut lane = 0usize;
    while lane < active_len {
        let absolute = output_offset.checked_add(lane)?;
        let output_row = absolute / storage_row_len;
        let row_start = output_row.checked_mul(storage_row_len)?;
        let row_offset = absolute.checked_sub(row_start)?;
        let row_remaining = storage_row_len.checked_sub(row_offset)?;
        let segment_len = row_remaining.min(active_len - lane);

        if row_offset < width {
            let copied_len = (width - row_offset).min(segment_len);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(storage_row_len)?;
            let rotated = narrow_rotated_row(source, source_start, width, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + copied_len {
                indices[destination_lane] =
                    u8::try_from(row_offset + destination_lane - lane).ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.shuffle(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
        }
        lane += segment_len;
    }
    Some(block)
}

/// Offset a narrow `I;16*` frame with masked vector blocks across rows.
///
/// Unlike ordinary byte images, the second half of every storage row is
/// intentionally zero in Pillow's historical `image8` path. Keeping that
/// zero fill inside the vector block lets 2×2 and other small typed images
/// take a real native SIMD path without inventing a scalar-only capability.
#[inline]
fn offset_luma16_narrow(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<(u64, u64)> {
    let expected_len = width.checked_mul(2)?.checked_mul(height)?;
    if source.len() != expected_len
        || output.len() != expected_len
        || width == 0
        || width >= 16
        || height == 0
        || yshift >= height
        || shift_bytes >= width
    {
        return None;
    }

    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let block = offset_luma16_narrow_block(
            source,
            output_offset,
            16,
            width,
            height,
            shift_bytes,
            yshift,
        )?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let block = offset_luma16_narrow_block(
            source,
            vector_len,
            scalar_tail,
            width,
            height,
            shift_bytes,
            yshift,
        )?;
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

pub(crate) fn execute_luma16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    xoffset: i32,
    yoffset: i32,
    big_endian: bool,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let (width, height, source) = checked_luma16_source(image, big_endian)?;
    if width == 0 || height == 0 {
        return Ok(record_luma16_output(image.clone(), 0, 0));
    }
    let (output, vector_blocks, scalar_tail) =
        offset_luma16_frame(&source, width, height, xoffset, yoffset)?;
    let native_output = luma16_native_bytes(output, big_endian);
    let samples = native_output
        .chunks_exact(2)
        .map(|bytes| u16::from_ne_bytes([bytes[0], bytes[1]]))
        .collect();
    let result = ImageBuffer::from_vec(width, height, samples).ok_or_else(offset_luma16_error)?;
    Ok(record_luma16_output(result, vector_blocks, scalar_tail))
}

fn checked_luma16_source(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    big_endian: bool,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let (width, height) = image.dimensions();
    let dimensions = crate::checked_dims::CheckedDims::new_allow_empty(width, height, 2)
        .map_err(|_| offset_luma16_error())?;
    if image.as_raw().len().checked_mul(2) != Some(dimensions.total_bytes()) {
        return Err(offset_luma16_error());
    }
    let source = image
        .as_raw()
        .iter()
        .flat_map(|sample| if big_endian { sample.to_be_bytes() } else { sample.to_ne_bytes() })
        .collect();
    Ok((width, height, source))
}

fn offset_luma16_frame(
    source: &[u8],
    width: u32,
    height: u32,
    xoffset: i32,
    yoffset: i32,
) -> Result<(Vec<u8>, u64, u64), PilError> {
    let width_usize = usize::try_from(width).map_err(|_| offset_luma16_error())?;
    let height_usize = usize::try_from(height).map_err(|_| offset_luma16_error())?;
    let row_bytes = width_usize.checked_mul(2).ok_or_else(offset_luma16_error)?;
    let total_bytes = source.len();
    let mut output = vec![0u8; total_bytes];
    let xshift = (-i64::from(xoffset)).rem_euclid(i64::from(width)) as usize;
    let yshift = (-i64::from(yoffset)).rem_euclid(i64::from(height)) as usize;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if width_usize < 16 {
        (vector_blocks, scalar_tail) = offset_luma16_narrow(
            &source,
            &mut output,
            width_usize,
            height_usize,
            xshift,
            yshift,
        )
        .ok_or_else(offset_luma16_error)?;
    } else {
        for output_y in 0..height_usize {
            let source_y = (output_y + yshift) % height_usize;
            let source_start = source_y
                .checked_mul(row_bytes)
                .ok_or_else(offset_luma16_error)?;
            let output_start = output_y
                .checked_mul(row_bytes)
                .ok_or_else(offset_luma16_error)?;
            let (blocks, tail) = offset_luma16_native_row(
                source
                    .get(
                        source_start
                            ..source_start
                                .checked_add(row_bytes)
                                .ok_or_else(offset_luma16_error)?,
                    )
                    .ok_or_else(offset_luma16_error)?,
                output
                    .get_mut(
                        output_start
                            ..output_start
                                .checked_add(row_bytes)
                                .ok_or_else(offset_luma16_error)?,
                    )
                    .ok_or_else(offset_luma16_error)?,
                width_usize,
                xshift,
            )
            .ok_or_else(offset_luma16_error)?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    Ok((output, vector_blocks, scalar_tail))
}

fn luma16_native_bytes(output: Vec<u8>, big_endian: bool) -> Vec<u8> {
    if big_endian {
        output
            .chunks_exact(2)
            .flat_map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]).to_ne_bytes())
            .collect()
    } else {
        output
    }
}

fn record_luma16_output(
    image: ImageBuffer<Luma<u16>, Vec<u16>>,
    vector_blocks: u64,
    scalar_tail: u64,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    image
}

fn offset_luma16_error() -> PilError {
    PilError::InternalError("Chops Offset input did not match its admitted I;16 layout".into())
}
