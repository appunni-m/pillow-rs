//! Native byte Offset kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;
use crate::raster::GenericImageView;
use wide::u8x16;

/// Load sixteen bytes from a circular row without splitting the operation
/// into scalar copies when the offset crosses the row boundary. The two
/// source vectors are selected with lane masks; the byte indices are control
/// data, not per-pixel arithmetic.
#[inline]
pub(super) fn circular_byte_block(source: &[u8], start: usize) -> Option<u8x16> {
    if source.len() < 16 || start >= source.len() {
        return None;
    }
    if start + 16 <= source.len() {
        return Some(u8x16::new(
            <[u8; 16]>::try_from(source.get(start..start + 16)?).ok()?,
        ));
    }

    let first_len = source.len() - start;
    let tail = u8x16::new(<[u8; 16]>::try_from(source.get(source.len() - 16..)?).ok()?);
    let head = u8x16::new(<[u8; 16]>::try_from(source.get(..16)?).ok()?);
    let mut tail_indices = [0u8; 16];
    let mut head_indices = [0u8; 16];
    let mut tail_mask = [0u8; 16];
    for lane in 0..16 {
        if lane < first_len {
            tail_indices[lane] = u8::try_from(16 - first_len + lane).ok()?;
            tail_mask[lane] = u8::MAX;
        } else {
            head_indices[lane] = u8::try_from(lane - first_len).ok()?;
        }
    }
    let tail_values = tail.swizzle_relaxed(u8x16::new(tail_indices));
    let head_values = head.swizzle_relaxed(u8x16::new(head_indices));
    let mask = u8x16::new(tail_mask);
    Some((tail_values & mask) | (head_values & (u8x16::splat(u8::MAX) ^ mask)))
}

/// Copy one wrapped offset row through complete native vector blocks. The
/// final incomplete store is the only scalar tail; the circular source load
/// remains vectorized even when the shift divides the row into two short
/// spans.
#[inline]
fn offset_native_row(source: &[u8], output: &mut [u8], shift_bytes: usize) -> Option<(u64, u64)> {
    if source.len() != output.len() || source.len() < 16 || shift_bytes >= source.len() {
        return None;
    }
    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let source_start = (shift_bytes + output_offset) % source.len();
        let block = circular_byte_block(source, source_start)?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let source_start = (shift_bytes + vector_len) % source.len();
        let block = circular_byte_block(source, source_start)?;
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Load one narrow row and rotate its byte lanes. The load may cross into the
/// following row; the swizzle mask only references the current row, so those
/// extra bytes are never observed. A final source row without sixteen bytes
/// remaining is the only place where a stack-padded load is necessary.
#[inline]
pub(super) fn narrow_rotated_row(
    source: &[u8],
    row_start: usize,
    row_len: usize,
    shift_bytes: usize,
) -> Option<u8x16> {
    if row_len == 0 || row_len >= 16 || shift_bytes >= row_len {
        return None;
    }
    let row_end = row_start.checked_add(row_len)?;
    let row = source.get(row_start..row_end)?;
    let loaded = if row_start.checked_add(16)?.le(&source.len()) {
        u8x16::new(<[u8; 16]>::try_from(source.get(row_start..row_start + 16)?).ok()?)
    } else {
        let mut padded = [0u8; 16];
        padded[..row_len].copy_from_slice(row);
        u8x16::new(padded)
    };
    let mut indices = [0u8; 16];
    for (lane, index) in indices.iter_mut().enumerate() {
        *index = u8::try_from((shift_bytes + lane) % row_len).ok()?;
    }
    Some(loaded.swizzle_relaxed(u8x16::new(indices)))
}

/// Process a narrow-row offset as a vector stream over the frame buffer.
/// Output blocks may contain the end of one row and the beginning of the next;
/// each segment is selected from a vectorized, independently rotated source
/// row before one contiguous vector store. This keeps the copy native without
/// allocating a row-sized staging buffer.
#[inline]
fn offset_narrow_bytes(
    source: &[u8],
    output: &mut [u8],
    row_len: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<(u64, u64)> {
    if source.len() != output.len()
        || row_len == 0
        || row_len >= 16
        || source.len() < 16
        || height == 0
        || yshift >= height
        || shift_bytes >= row_len
    {
        return None;
    }
    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let mut block = u8x16::splat(0);
        let mut lane = 0usize;
        while lane < 16 {
            let absolute = output_offset.checked_add(lane)?;
            if absolute >= output.len() {
                break;
            }
            let output_row = absolute / row_len;
            let row_start = output_row.checked_mul(row_len)?;
            let row_offset = absolute - row_start;
            let row_end_lane = (row_len - row_offset).min(16 - lane);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(row_len)?;
            let rotated = narrow_rotated_row(source, source_start, row_len, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + row_end_lane {
                indices[destination_lane] =
                    u8::try_from(row_offset + destination_lane - lane).ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.swizzle_relaxed(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
            lane += row_end_lane;
        }
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let mut block = u8x16::splat(0);
        let mut lane = 0usize;
        while lane < scalar_tail {
            let absolute = vector_len.checked_add(lane)?;
            let output_row = absolute / row_len;
            let row_start = output_row.checked_mul(row_len)?;
            let row_offset = absolute - row_start;
            let row_end_lane = (row_len - row_offset).min(scalar_tail - lane);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(row_len)?;
            let rotated = narrow_rotated_row(source, source_start, row_len, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + row_end_lane {
                indices[destination_lane] =
                    u8::try_from(row_offset + destination_lane - lane).ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.swizzle_relaxed(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
            lane += row_end_lane;
        }
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Offset one exact-mode byte buffer through the shared architecture kernel.
/// The mode owner supplies its typed buffer, so channel width comes from the
/// pixel type and output identity remains with that owner.
pub(crate) fn execute_bytes<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    xoffset: i32,
    yoffset: i32,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (width, height) = image.dimensions();
    let channels = usize::from(P::CHANNEL_COUNT);
    let dimensions =
        CheckedDims::new_allow_empty(width, height, channels).map_err(|_| offset_layout_error())?;
    if image.as_raw().len() != dimensions.total_bytes() {
        return Err(offset_layout_error());
    }
    if width == 0 || height == 0 {
        return Ok(record_output(image.clone(), 0, 0));
    }
    if dimensions.total_bytes() < 16 {
        return Err(offset_layout_error());
    }
    let width_usize = usize::try_from(width).map_err(|_| offset_layout_error())?;
    let height_usize = usize::try_from(height).map_err(|_| offset_layout_error())?;
    let row_len = width_usize.checked_mul(channels).ok_or_else(offset_layout_error)?;
    let total_len = dimensions.total_bytes();
    let source = image.as_raw();
    let mut output = vec![0u8; total_len];
    let xshift = (-i64::from(xoffset)).rem_euclid(i64::from(width)) as usize;
    let yshift = (-i64::from(yoffset)).rem_euclid(i64::from(height)) as usize;
    let shift_bytes = xshift
        .checked_mul(channels)
        .ok_or_else(offset_layout_error)?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if row_len < 16 {
        let (blocks, tail) = offset_narrow_bytes(
            source,
            &mut output,
            row_len,
            height_usize,
            shift_bytes,
            yshift,
        )
        .ok_or_else(offset_layout_error)?;
        vector_blocks = blocks;
        scalar_tail = tail;
    } else {
        for output_y in 0..height_usize {
            let source_y = (output_y + yshift) % height_usize;
            let source_row = source_y
                .checked_mul(row_len)
                .ok_or_else(offset_layout_error)?;
            let output_row = output_y
                .checked_mul(row_len)
                .ok_or_else(offset_layout_error)?;
            let source_end = source_row
                .checked_add(row_len)
                .ok_or_else(offset_layout_error)?;
            let output_end = output_row
                .checked_add(row_len)
                .ok_or_else(offset_layout_error)?;
            let source_slice = source
                .get(source_row..source_end)
                .ok_or_else(offset_layout_error)?;
            let output_slice = output
                .get_mut(output_row..output_end)
                .ok_or_else(offset_layout_error)?;
            let (blocks, tail) =
                offset_native_row(source_slice, output_slice, shift_bytes)
                    .ok_or_else(offset_layout_error)?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    let mut result = ImageBuffer::from_raw(width, height, output).ok_or_else(offset_layout_error)?;
    result
        .remap_hidden_pixel_lanes_from(image, |index| {
            let output_x = index % width_usize;
            let output_y = index / width_usize;
            let source_x = (output_x + xshift) % width_usize;
            let source_y = (output_y + yshift) % height_usize;
            source_y * width_usize + source_x
        })
        .ok_or_else(offset_layout_error)?;
    Ok(record_output(result, vector_blocks, scalar_tail))
}

fn record_output<P: Pixel<Subpixel = u8>>(
    image: ImageBuffer<P, Vec<u8>>,
    vector_blocks: u64,
    scalar_tail: u64,
) -> ImageBuffer<P, Vec<u8>> {
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    image
}

fn offset_layout_error() -> PilError {
    PilError::InternalError("Chops Offset input did not match its admitted native layout".into())
}
