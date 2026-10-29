//! Native sample copying for exact-mode ImageOps.expand implementations.

use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Pixel, Primitive};

pub(crate) fn expanded_dimensions(
    width: u32,
    height: u32,
    border: u32,
    bytes_per_pixel: u8,
) -> Result<(u32, u32), PilError> {
    let border_twice = border
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("expand border overflows dimensions".into()))?;
    let width = width
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image width overflows".into()))?;
    let height = height
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image height overflows".into()))?;
    crate::checked_dims::CheckedDims::new(width, height, bytes_per_pixel)?;
    Ok((width, height))
}

pub(crate) fn expand_samples<T: Copy>(
    source: &[T],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    border: u32,
    channels: usize,
    fill: &[T],
) -> Result<Vec<T>, PilError> {
    let source_stride = (source_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("expand source stride overflows".into()))?;
    let expected_source_len = source_stride
        .checked_mul(source_height as usize)
        .ok_or_else(|| PilError::DimensionError("expand source size overflows".into()))?;
    if channels == 0 || fill.len() != channels || source.len() != expected_source_len {
        return Err(PilError::InternalError(
            "Expand samples did not match their native mode".into(),
        ));
    }
    let output_stride = (output_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("expanded image stride overflows".into()))?;
    let output_len = output_stride
        .checked_mul(output_height as usize)
        .ok_or_else(|| PilError::DimensionError("expanded image size overflows".into()))?;
    if border == 0 {
        return Ok(source.to_vec());
    }

    let border = border as usize;
    let source_end_y = border + source_height as usize;
    let fill_row = fill.repeat(output_width as usize);
    let fill_sides = fill.repeat(border);
    let mut output = Vec::with_capacity(output_len);
    for y in 0..output_height as usize {
        if y < border || y >= source_end_y {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - border;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Expand output did not match its declared dimensions".into(),
        ));
    }
    Ok(output)
}

pub(crate) fn expand_buffer<P, T>(
    input: &ImageBuffer<P, Vec<T>>,
    border: u32,
    bytes_per_pixel: u8,
    channels: usize,
    fill: &[T],
) -> Result<ImageBuffer<P, Vec<T>>, PilError>
where
    P: Pixel<Subpixel = T>,
    T: Copy + Primitive,
{
    let (width, height) = expanded_dimensions(
        input.width(),
        input.height(),
        border,
        bytes_per_pixel,
    )?;
    let samples = expand_samples(
        input.as_raw(),
        input.width(),
        input.height(),
        width,
        height,
        border,
        channels,
        fill,
    )?;
    ImageBuffer::from_raw(width, height, samples)
        .ok_or_else(|| PilError::InternalError("Expand output dimensions were invalid".into()))
}
