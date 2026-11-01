//! Exact native-sample canvas allocation and row copying.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::{
    self, PadFrame,
};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

/// Pads one already-resized native buffer without changing its pixel type.
pub(crate) fn pad<P, T>(
    source: &ImageBuffer<P, Vec<T>>,
    frame: PadFrame,
    fill: [T; 4],
) -> Result<ImageBuffer<P, Vec<T>>, PilError>
where
    P: Pixel<Subpixel = T>,
    T: Copy,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    let pixel_bytes = channels
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| u8::try_from(bytes).ok())
        .ok_or_else(|| PilError::DimensionError("Pad pixel size overflow".into()))?;
    let dims = CheckedDims::new_allow_empty(frame.width, frame.height, pixel_bytes)?;
    let sample_count = dims.total_bytes() / std::mem::size_of::<T>();
    let width = usize::try_from(frame.width)
        .map_err(|_| PilError::DimensionError("Pad output width overflow".into()))?;
    let height = usize::try_from(frame.height)
        .map_err(|_| PilError::DimensionError("Pad output height overflow".into()))?;
    let row_samples = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("Pad output row overflow".into()))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(sample_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    output.resize(sample_count, fill[0]);
    for pixel in output.chunks_exact_mut(channels) {
        pixel.copy_from_slice(&fill[..channels]);
    }

    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::DimensionError("Pad source width overflow".into()))?;
    let source_height = usize::try_from(source.height())
        .map_err(|_| PilError::DimensionError("Pad source height overflow".into()))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("Pad source row overflow".into()))?;
    let source_count = source_stride
        .checked_mul(source_height)
        .ok_or_else(|| PilError::DimensionError("Pad source buffer overflow".into()))?;
    if source_count != source.as_raw().len() {
        return Err(PilError::InternalError("Pad source buffer shape mismatch".into()));
    }

    let copy_width = usize::try_from(frame.content_width.min(frame.width))
        .map_err(|_| PilError::DimensionError("Pad content width overflow".into()))?;
    let copy_height = usize::try_from(frame.content_height.min(frame.height))
        .map_err(|_| PilError::DimensionError("Pad content height overflow".into()))?;
    let offset_x = usize::try_from(frame.offset_x)
        .map_err(|_| PilError::DimensionError("Pad horizontal offset overflow".into()))?;
    let offset_y = usize::try_from(frame.offset_y)
        .map_err(|_| PilError::DimensionError("Pad vertical offset overflow".into()))?;
    let copy_samples = copy_width
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("Pad content row overflow".into()))?;
    if offset_x.checked_add(copy_width).map_or(true, |end| end > width)
        || offset_y.checked_add(copy_height).map_or(true, |end| end > height)
        || copy_width > source_width
        || copy_height > source_height
    {
        return Err(PilError::InternalError("Pad content window exceeds its image".into()));
    }

    for y in 0..copy_height {
        let source_start = y * source_stride;
        let target_start = (offset_y + y) * row_samples + offset_x * channels;
        output[target_start..target_start + copy_samples]
            .copy_from_slice(&source.as_raw()[source_start..source_start + copy_samples]);
    }

    ImageBuffer::from_raw(frame.width, frame.height, output)
        .ok_or_else(|| PilError::InternalError("Pad output buffer shape mismatch".into()))
}

/// Builds a vertical Pad by appending complete fill rows around native samples.
pub(crate) fn pad_vertical_append<P, T>(
    source: &ImageBuffer<P, Vec<T>>,
    frame: PadFrame,
    fill: [T; 4],
) -> Result<Option<ImageBuffer<P, Vec<T>>>, PilError>
where
    P: Pixel<Subpixel = T>,
    T: Copy,
{
    let Some(rows) = dimensions::full_width_vertical_rows(source.dimensions(), frame) else {
        return Ok(None);
    };
    let channels = usize::from(P::CHANNEL_COUNT);
    let pixel_bytes = channels
        .checked_mul(std::mem::size_of::<T>())
        .and_then(|bytes| u8::try_from(bytes).ok())
        .ok_or_else(|| PilError::DimensionError("Pad pixel size overflow".into()))?;
    let dims = CheckedDims::new_allow_empty(frame.width, frame.height, pixel_bytes)?;
    let sample_count = dims.total_bytes() / std::mem::size_of::<T>();
    let width = usize::try_from(frame.width)
        .map_err(|_| PilError::DimensionError("Pad output width overflow".into()))?;
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::DimensionError("Pad source width overflow".into()))?;
    let source_height = usize::try_from(source.height())
        .map_err(|_| PilError::DimensionError("Pad source height overflow".into()))?;
    let row_samples = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("Pad output row overflow".into()))?;
    let source_samples = source_width
        .checked_mul(channels)
        .and_then(|stride| stride.checked_mul(source_height))
        .ok_or_else(|| PilError::DimensionError("Pad source buffer overflow".into()))?;
    if source_samples != source.as_raw().len() {
        return Err(PilError::InternalError("Pad source buffer shape mismatch".into()));
    }
    let fill_row = fill[..channels].repeat(width);
    if fill_row.len() != row_samples {
        return Err(PilError::InternalError("Pad fill row shape mismatch".into()));
    }

    let mut output = Vec::new();
    output
        .try_reserve_exact(sample_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for _ in 0..rows.top {
        output.extend_from_slice(&fill_row);
    }
    output.extend_from_slice(source.as_raw());
    for _ in 0..rows.bottom {
        output.extend_from_slice(&fill_row);
    }
    if output.len() != sample_count {
        return Err(PilError::InternalError("Pad output buffer shape mismatch".into()));
    }
    ImageBuffer::from_raw(frame.width, frame.height, output)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("Pad output buffer shape mismatch".into()))
}
