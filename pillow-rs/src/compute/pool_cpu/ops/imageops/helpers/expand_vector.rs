//! Shared 128-bit byte fill/copy kernel for exact-mode ImageOps.expand plans.

use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};
use wide::u8x16;

pub(crate) fn expand_buffer<P>(
    input: &ImageBuffer<P, Vec<u8>>,
    border: u32,
    channels: u8,
    fill: (u8, u8, u8, u8),
) -> Result<(ImageBuffer<P, Vec<u8>>, u64, u64), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let (output_width, output_height) = super::expand::expanded_dimensions(
        input.width(),
        input.height(),
        border,
        channels,
    )?;
    let source_stride = (input.width() as usize)
        .checked_mul(usize::from(channels))
        .ok_or_else(|| PilError::DimensionError("expand source stride overflows".into()))?;
    let expected_source_len = source_stride
        .checked_mul(input.height() as usize)
        .ok_or_else(|| PilError::DimensionError("expand source size overflows".into()))?;
    if channels == 0 || input.as_raw().len() != expected_source_len {
        return Err(PilError::InternalError(
            "Expand samples did not match their native mode".into(),
        ));
    }
    let output_stride = (output_width as usize)
        .checked_mul(usize::from(channels))
        .ok_or_else(|| PilError::DimensionError("expanded image stride overflows".into()))?;
    let output_len = output_stride
        .checked_mul(output_height as usize)
        .ok_or_else(|| PilError::DimensionError("expanded image size overflows".into()))?;

    let (samples, vector_blocks, scalar_tail) = if border == 0 {
        vector_copy(input.as_raw())?
    } else {
        match channels {
            1 => expand_luma(
                input.as_raw(),
                input.width(),
                input.height(),
                output_width,
                output_height,
                output_stride,
                output_len,
                border,
                fill,
            )?,
            2 => expand_luma_alpha(
                input.as_raw(),
                input.width(),
                input.height(),
                output_height,
                output_stride,
                output_len,
                border,
                fill,
            )?,
            _ => {
                return Err(PilError::InternalError(
                    "Expand vector strategy received an ineligible native layout".into(),
                ));
            }
        }
    };
    if samples.len() != output_len {
        return Err(PilError::InternalError(
            "SIMD expand output shape mismatch".into(),
        ));
    }
    let output = ImageBuffer::from_raw(output_width, output_height, samples)
        .ok_or_else(|| PilError::InternalError("Expand output dimensions were invalid".into()))?;
    Ok((output, vector_blocks, scalar_tail))
}

fn expand_luma(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    output_stride: usize,
    output_len: usize,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Result<(Vec<u8>, u64, u64), PilError> {
    let mut fill_row = vec![0; output_width as usize];
    let (mut vector_blocks, mut scalar_tail) = fill_vector_row(&mut fill_row, fill, 1)?;
    let mut fill_sides = vec![0; border as usize];
    let (side_blocks, side_tail) = fill_vector_row(&mut fill_sides, fill, 1)?;
    vector_blocks = vector_blocks.saturating_add(side_blocks);
    scalar_tail = scalar_tail.saturating_add(side_tail);

    let source_stride = source_width as usize;
    let source_end_y = (border as usize)
        .checked_add(source_height as usize)
        .ok_or_else(|| PilError::DimensionError("expand source rows overflow".into()))?;
    let mut output = Vec::with_capacity(output_len);
    for y in 0..output_height as usize {
        if y < border as usize || y >= source_end_y {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - border as usize;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }
    if output.len() != output_len || output_stride != output_width as usize {
        return Err(PilError::InternalError("SIMD expand L output shape mismatch".into()));
    }
    Ok((output, vector_blocks, scalar_tail))
}

fn expand_luma_alpha(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    output_height: u32,
    output_stride: usize,
    output_len: usize,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Result<(Vec<u8>, u64, u64), PilError> {
    let border = border as usize;
    let border_bytes = border
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("expand border stride overflows".into()))?;
    let source_stride = (source_width as usize)
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("expand source stride overflows".into()))?;
    let source_end_y = border
        .checked_add(source_height as usize)
        .ok_or_else(|| PilError::DimensionError("expand source rows overflow".into()))?;
    let mut output = vec![0; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..output_height as usize {
        let row_start = y * output_stride;
        let row = &mut output[row_start..row_start + output_stride];
        if y < border || y >= source_end_y {
            add_counts(&mut vector_blocks, &mut scalar_tail, fill_vector_row(row, fill, 2)?);
            continue;
        }
        add_counts(
            &mut vector_blocks,
            &mut scalar_tail,
            fill_vector_row(&mut row[..border_bytes], fill, 2)?,
        );
        let source_y = y - border;
        let source_start = source_y * source_stride;
        let source_end = source_start + source_stride;
        let copy_start = border_bytes;
        let copy_end = copy_start + source_stride;
        add_counts(
            &mut vector_blocks,
            &mut scalar_tail,
            copy_bytes(&source[source_start..source_end], &mut row[copy_start..copy_end])?,
        );
        add_counts(
            &mut vector_blocks,
            &mut scalar_tail,
            fill_vector_row(&mut row[copy_end..], fill, 2)?,
        );
    }
    Ok((output, vector_blocks, scalar_tail))
}

fn fill_vector_row(
    row: &mut [u8],
    fill: (u8, u8, u8, u8),
    channels: u8,
) -> Result<(u64, u64), PilError> {
    if channels == 1 {
        let pattern = u8x16::splat(fill.0).to_array();
        let vector_len = row.len() / pattern.len() * pattern.len();
        for chunk in row[..vector_len].chunks_exact_mut(pattern.len()) {
            chunk.copy_from_slice(&pattern);
        }
        let scalar_tail = row.len() - vector_len;
        row[vector_len..].copy_from_slice(&pattern[..scalar_tail]);
        return Ok((
            (vector_len / pattern.len() + usize::from(scalar_tail != 0)) as u64,
            scalar_tail as u64,
        ));
    }
    if channels != 2 {
        return Err(PilError::InternalError(
            "Expand vector fill received an unsupported channel count".into(),
        ));
    }
    let vector_len = row.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(16) {
        let block = u8x16::new(std::array::from_fn(|lane| {
            expand_fill_sample(fill, start + lane)
        }));
        row[start..start + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = row.len() - vector_len;
    if scalar_tail != 0 {
        let block = u8x16::new(std::array::from_fn(|lane| {
            expand_fill_sample(fill, vector_len + lane)
        }));
        row[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Ok((vector_blocks, scalar_tail as u64))
}

fn expand_fill_sample(fill: (u8, u8, u8, u8), index: usize) -> u8 {
    if index.is_multiple_of(2) { fill.0 } else { fill.3 }
}

fn copy_bytes(source: &[u8], output: &mut [u8]) -> Result<(u64, u64), PilError> {
    crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(source, output)
        .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))
}

fn vector_copy(source: &[u8]) -> Result<(Vec<u8>, u64, u64), PilError> {
    let mut output = vec![0; source.len()];
    let (vector_blocks, scalar_tail) = copy_bytes(source, &mut output)?;
    Ok((output, vector_blocks, scalar_tail))
}

fn add_counts(total: &mut u64, tail: &mut u64, next: (u64, u64)) {
    *total = total.saturating_add(next.0);
    *tail = tail.saturating_add(next.1);
}
