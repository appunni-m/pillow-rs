//! Native 16-bit sample Offset kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::ImageBuffer;
use crate::raster::color::Luma;
use wide::{ShuffleExt, u16x8};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Rotate one row of complete 16-bit samples, vectorizing each full block.
#[inline]
fn offset_luma16_row(
    source: &[u16],
    output: &mut [u16],
    shift_samples: usize,
) -> Option<(u64, u64)> {
    if source.len() != output.len() || source.is_empty() || shift_samples >= source.len() {
        return None;
    }

    let vector_len = output.len() / 8 * 8;
    let mut vector_blocks = 0u64;
    for output_start in (0..vector_len).step_by(8) {
        let source_start = (shift_samples + output_start) % source.len();
        let block = offset_luma16_vector_block(source, source_start)?;
        output[output_start..output_start + 8].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let scalar_start = vector_len;
    for output_index in scalar_start..output.len() {
        output[output_index] = source[(shift_samples + output_index) % source.len()];
    }
    let scalar_tail = output.len() - scalar_start;
    Some((vector_blocks, u64::try_from(scalar_tail).ok()?))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Load eight samples from a row, joining the wrapped tail and head if needed.
#[inline]
fn offset_luma16_vector_block(source: &[u16], source_start: usize) -> Option<u16x8> {
    let remaining = source.len().checked_sub(source_start)?;
    if remaining >= 8 {
        return Some(u16x8::new(
            <[u16; 8]>::try_from(source.get(source_start..source_start.checked_add(8)?)?).ok()?,
        ));
    }

    let head_len = remaining;
    let tail_len = 8usize.checked_sub(head_len)?;
    if tail_len > source.len() {
        return None;
    }

    let mut head = [0u16; 8];
    let mut tail = [0u16; 8];
    head[..head_len].copy_from_slice(source.get(source_start..)?);
    tail[..tail_len].copy_from_slice(source.get(..tail_len)?);
    let mut indices = [0u16; 8];
    for (lane, index) in indices.iter_mut().enumerate() {
        let source_lane = if lane < head_len {
            lane
        } else {
            8 + lane - head_len
        };
        *index = u16::try_from(source_lane).ok()?;
    }
    Some([u16x8::new(head), u16x8::new(tail)].shuffle(u16x8::new(indices)))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_luma16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    xoffset: i32,
    yoffset: i32,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    // Offset wraps whole pixels. Pillow 12.2 left half of each I;16 row
    // unwritten; Pillow 13.0 fixed that output contract.
    let (width, height) = image.dimensions();
    let dimensions =
        CheckedDims::new_allow_empty(width, height, 2).map_err(|_| offset_luma16_error())?;
    if image.as_raw().len() != dimensions.total_pixels() {
        return Err(offset_luma16_error());
    }
    if width == 0 || height == 0 {
        return Ok(record_luma16_output(image.clone(), 0, 0));
    }

    let width_usize = usize::try_from(width).map_err(|_| offset_luma16_error())?;
    let height_usize = usize::try_from(height).map_err(|_| offset_luma16_error())?;
    let xshift = usize::try_from((-i64::from(xoffset)).rem_euclid(i64::from(width)))
        .map_err(|_| offset_luma16_error())?;
    let yshift = usize::try_from((-i64::from(yoffset)).rem_euclid(i64::from(height)))
        .map_err(|_| offset_luma16_error())?;
    let mut output = vec![0u16; dimensions.total_pixels()];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for output_y in 0..height_usize {
        let source_y = (output_y + yshift) % height_usize;
        let source_start = source_y
            .checked_mul(width_usize)
            .ok_or_else(offset_luma16_error)?;
        let output_start = output_y
            .checked_mul(width_usize)
            .ok_or_else(offset_luma16_error)?;
        let (blocks, tail) = offset_luma16_row(
            image
                .as_raw()
                .get(
                    source_start
                        ..source_start
                            .checked_add(width_usize)
                            .ok_or_else(offset_luma16_error)?,
                )
                .ok_or_else(offset_luma16_error)?,
            output
                .get_mut(
                    output_start
                        ..output_start
                            .checked_add(width_usize)
                            .ok_or_else(offset_luma16_error)?,
                )
                .ok_or_else(offset_luma16_error)?,
            xshift,
        )
        .ok_or_else(offset_luma16_error)?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }

    let result = ImageBuffer::from_vec(width, height, output).ok_or_else(offset_luma16_error)?;
    Ok(record_luma16_output(result, vector_blocks, scalar_tail))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn offset_luma16_error() -> PilError {
    PilError::InternalError("Chops Offset input did not match its admitted I;16 layout".into())
}
