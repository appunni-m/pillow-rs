//! Native sample copying for exact-mode ImageOps.expand implementations.

use crate::error::PilError;
use crate::pipeline::ExpandMargins;
use crate::raster::{GenericImageView, ImageBuffer, Pixel, Primitive};

pub(crate) fn expanded_dimensions(
    width: u32,
    height: u32,
    border: ExpandMargins,
    bytes_per_pixel: u8,
) -> Result<(u32, u32), PilError> {
    let width = i128::from(width) + i128::from(border.left) + i128::from(border.right);
    let height = i128::from(height) + i128::from(border.top) + i128::from(border.bottom);
    let width = u32::try_from(width)
        .map_err(|_| PilError::DimensionError("expanded image width is invalid".into()))?;
    let height = u32::try_from(height)
        .map_err(|_| PilError::DimensionError("expanded image height is invalid".into()))?;
    crate::checked_dims::CheckedDims::new_allow_empty(width, height, bytes_per_pixel)?;
    Ok((width, height))
}

pub(crate) fn expand_samples<T: Copy>(
    source: &[T],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    border: ExpandMargins,
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
    if border.uniform_nonnegative() == Some(0) {
        return Ok(source.to_vec());
    }

    if let Some(border) = border.uniform_nonnegative() {
        return expand_uniform_samples(
            source,
            source_height,
            output_width,
            output_height,
            border,
            source_stride,
            output_len,
            fill,
        );
    }

    expand_margin_samples(
        source,
        source_width,
        source_height,
        output_width,
        output_height,
        border,
        channels,
        fill,
        source_stride,
        output_stride,
        output_len,
    )
}

fn expand_uniform_samples<T: Copy>(
    source: &[T],
    source_height: u32,
    output_width: u32,
    output_height: u32,
    border: u32,
    source_stride: usize,
    output_len: usize,
    fill: &[T],
) -> Result<Vec<T>, PilError> {
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

fn expand_margin_samples<T: Copy>(
    source: &[T],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    border: ExpandMargins,
    channels: usize,
    fill: &[T],
    source_stride: usize,
    output_stride: usize,
    output_len: usize,
) -> Result<Vec<T>, PilError> {
    let mut output = fill.repeat(output_len / channels);
    for output_y in 0..output_height {
        let source_y = i128::from(output_y) - i128::from(border.top);
        if !(0..i128::from(source_height)).contains(&source_y) {
            continue;
        }

        let destination_start = i128::from(border.left).max(0).min(i128::from(output_width));
        let destination_end = (i128::from(border.left) + i128::from(source_width))
            .max(0)
            .min(i128::from(output_width));
        if destination_end <= destination_start {
            continue;
        }

        let source_start = source_y as usize * source_stride
            + (destination_start - i128::from(border.left)) as usize * channels;
        let copy_len = (destination_end - destination_start) as usize * channels;
        let output_start = output_y as usize * output_stride + destination_start as usize * channels;
        output[output_start..output_start + copy_len]
            .copy_from_slice(&source[source_start..source_start + copy_len]);
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
    border: ExpandMargins,
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
