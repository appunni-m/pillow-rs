//! Native-byte nearest-neighbor gather and packed output blocks.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use wide::u8x16;

const VECTOR_BYTES: usize = 16;

pub(crate) fn execute<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    output_size: (u32, u32),
) -> Result<Vec<u8>, PilError> {
    execute_for_bounds::<CHANNELS>(source, source_size, output_size, None)
}

pub(crate) fn execute_boxed<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    output_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> Result<Vec<u8>, PilError> {
    execute_for_bounds::<CHANNELS>(source, source_size, output_size, Some(bounds))
}

fn execute_for_bounds<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    output_size: (u32, u32),
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<Vec<u8>, PilError> {
    if !(1..=4).contains(&CHANNELS) {
        return Err(invalid_nearest_layout());
    }
    let source_dims = CheckedDims::new(source_size.0, source_size.1, CHANNELS as u8)?;
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "native nearest Resize received an invalid source buffer".into(),
        ));
    }
    let output_dims = CheckedDims::new(output_size.0, output_size.1, CHANNELS as u8)?;
    let (x_bounds, y_bounds) = match bounds {
        Some((left, top, right, bottom)) => (Some((left, right)), Some((top, bottom))),
        None => (None, None),
    };
    let x_indices = resize_indices(source_size.0, output_size.0, x_bounds)
        .ok_or_else(invalid_nearest_layout)?;
    let y_indices = resize_indices(source_size.1, output_size.1, y_bounds)
        .ok_or_else(invalid_nearest_layout)?;
    let mut output = output_dims.alloc_buffer();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_size.0 as usize)
            .and_then(|offset| offset.checked_mul(CHANNELS))
            .ok_or_else(invalid_nearest_layout)?;
        let output_row = output_y
            .checked_mul(output_size.0 as usize)
            .and_then(|offset| offset.checked_mul(CHANNELS))
            .ok_or_else(invalid_nearest_layout)?;
        let (row_blocks, row_tail) = write_row::<CHANNELS>(
            source,
            source_row,
            &x_indices,
            output_size.0 as usize,
            &mut output,
            output_row,
        )
        .ok_or_else(invalid_nearest_layout)?;
        vector_blocks = vector_blocks.saturating_add(row_blocks);
        scalar_tail = scalar_tail.saturating_add(row_tail);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(output)
}

fn resize_indices(
    source_size: u32,
    output_size: u32,
    bounds: Option<(f64, f64)>,
) -> Option<Vec<usize>> {
    match bounds {
        Some((start, end)) => boxed_nearest_indices(source_size, output_size, start, end),
        None => nearest_indices(source_size, output_size),
    }
}

fn nearest_indices(source_size: u32, output_size: u32) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 {
        return None;
    }
    let scale = f64::from(source_size) / f64::from(output_size);
    let mut position = scale * 0.5;
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    for _ in 0..output_size {
        let index = position as u32;
        indices.push(if index >= source_size {
            usize::try_from(source_size - 1).ok()?
        } else {
            usize::try_from(index).ok()?
        });
        position += scale;
    }
    Some(indices)
}

fn boxed_nearest_indices(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 || !box_start.is_finite() || !box_end.is_finite() {
        return None;
    }
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    let last = f64::from(source_size - 1);
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    let mut coordinate = box_start + scale * 0.5;
    for _ in 0..output_size {
        indices.push(coordinate.floor().clamp(0.0, last) as usize);
        coordinate += scale;
    }
    Some(indices)
}

fn write_row<const CHANNELS: usize>(
    source: &[u8],
    source_row: usize,
    x_indices: &[usize],
    output_width: usize,
    output: &mut [u8],
    output_row: usize,
) -> Option<(u64, u64)> {
    let pixels_per_vector = VECTOR_BYTES / CHANNELS;
    let vector_width = output_width / pixels_per_vector * pixels_per_vector;
    let block_bytes = pixels_per_vector.checked_mul(CHANNELS)?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if output_width < pixels_per_vector {
        let values = vector_block::<CHANNELS>(source, source_row, x_indices, 0, output_width)?;
        let output_bytes = output_width.checked_mul(CHANNELS)?;
        output
            .get_mut(output_row..output_row.checked_add(output_bytes)?)?
            .copy_from_slice(&values[..output_bytes]);
        return Some((1, 0));
    }
    for output_x in (0..vector_width).step_by(pixels_per_vector) {
        let values = vector_block::<CHANNELS>(
            source,
            source_row,
            x_indices,
            output_x,
            pixels_per_vector,
        )?;
        let output_start = output_row.checked_add(output_x.checked_mul(CHANNELS)?)?;
        output
            .get_mut(output_start..output_start.checked_add(block_bytes)?)?
            .copy_from_slice(&values[..block_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    for output_x in vector_width..output_width {
        let source_start = source_row.checked_add(x_indices.get(output_x)?.checked_mul(CHANNELS)?)?;
        let output_start = output_row.checked_add(output_x.checked_mul(CHANNELS)?)?;
        let end = source_start.checked_add(CHANNELS)?;
        output
            .get_mut(output_start..output_start.checked_add(CHANNELS)?)?
            .copy_from_slice(source.get(source_start..end)?);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

fn vector_block<const CHANNELS: usize>(
    source: &[u8],
    source_row: usize,
    x_indices: &[usize],
    output_x: usize,
    pixel_count: usize,
) -> Option<[u8; VECTOR_BYTES]> {
    let first_source = x_indices.get(output_x)?.checked_mul(CHANNELS)?;
    let last_output = output_x.checked_add(pixel_count)?.checked_sub(1)?;
    let last_source = x_indices
        .get(last_output)?
        .checked_mul(CHANNELS)?
        .checked_add(CHANNELS)?;
    let span = last_source.checked_sub(first_source)?;
    if span > VECTOR_BYTES {
        return gather_block::<CHANNELS>(source, source_row, x_indices, output_x, pixel_count);
    }
    let source_start = source_row.checked_add(first_source)?;
    let mut source_bytes = [0u8; VECTOR_BYTES];
    source_bytes[..span].copy_from_slice(source.get(source_start..source_start.checked_add(span)?)?);
    let mut indices = [0u8; VECTOR_BYTES];
    for pixel in 0..pixel_count {
        let source_pixel = x_indices
            .get(output_x.checked_add(pixel)?)?
            .checked_mul(CHANNELS)?
            .checked_sub(first_source)?;
        for channel in 0..CHANNELS {
            let lane = pixel.checked_mul(CHANNELS)?.checked_add(channel)?;
            indices[lane] = u8::try_from(source_pixel.checked_add(channel)?).ok()?;
        }
    }
    Some(u8x16::new(source_bytes).shuffle(u8x16::new(indices)).to_array())
}

fn gather_block<const CHANNELS: usize>(
    source: &[u8],
    source_row: usize,
    x_indices: &[usize],
    output_x: usize,
    pixel_count: usize,
) -> Option<[u8; VECTOR_BYTES]> {
    let mut output = [0u8; VECTOR_BYTES];
    for pixel in 0..pixel_count {
        let source_pixel = x_indices.get(output_x.checked_add(pixel)?)?.checked_mul(CHANNELS)?;
        let source_start = source_row.checked_add(source_pixel)?;
        let destination_start = pixel.checked_mul(CHANNELS)?;
        let channel_end = destination_start.checked_add(CHANNELS)?;
        output
            .get_mut(destination_start..channel_end)?
            .copy_from_slice(source.get(source_start..source_start.checked_add(CHANNELS)?)?);
    }
    Some(output)
}

fn invalid_nearest_layout() -> PilError {
    PilError::InternalError("native nearest Resize plan did not match its byte layout".into())
}
