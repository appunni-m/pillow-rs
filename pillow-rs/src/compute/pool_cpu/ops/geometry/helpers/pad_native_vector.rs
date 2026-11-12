//! Portable 128-bit fill and copy kernel for native-byte Pad canvases.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use wide::u8x16;

/// Fills border spans and copies the resized source in its exact byte carrier.
pub(crate) fn pad<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    frame: super::pad_dimensions::PadFrame,
    fill: [u8; 4],
) -> Result<(ImageBuffer<P, Vec<u8>>, u64, u64), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    if !(1..=4).contains(&channels) {
        return Err(PilError::InternalError(
            "Pad vector mode has invalid channels".into(),
        ));
    }
    let output_dims = CheckedDims::new_allow_empty(frame.width, frame.height, channels as u8)?;
    let output_len = output_dims.total_bytes();
    let target_stride = checked_product(
        usize::try_from(frame.width)
            .map_err(|_| PilError::DimensionError("Pad output width overflow".into()))?,
        channels,
        "Pad output row overflow",
    )?;
    if output_len == 0 {
        return image_from_raw(frame.width, frame.height, Vec::new()).map(|image| (image, 0, 0));
    }

    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::DimensionError("Pad source width overflow".into()))?;
    let source_height = usize::try_from(source.height())
        .map_err(|_| PilError::DimensionError("Pad source height overflow".into()))?;
    let source_stride = checked_product(source_width, channels, "Pad source row overflow")?;
    let source_len = checked_product(source_stride, source_height, "Pad source size overflow")?;
    if source_len != source.as_raw().len() {
        return Err(PilError::InternalError(
            "Pad source buffer shape mismatch".into(),
        ));
    }

    let window = content_window(
        frame,
        channels,
        target_stride,
        source_stride,
        source_width,
        source_height,
    )?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    output.resize(output_len, 0);
    if window.is_identity(source_width, source_height, frame, target_stride) {
        let (blocks, tail) = copy_native_bytes(source.as_raw(), &mut output)
            .ok_or_else(|| PilError::InternalError("Pad vector copy shape mismatch".into()))?;
        return image_from_raw(frame.width, frame.height, output)
            .map(|image| (image, blocks, tail));
    }

    let fill = &fill[..channels];
    let fill_is_zero = fill.iter().all(|sample| *sample == 0);
    let mut counts = VectorCounts::default();
    for (output_y, row) in output.chunks_exact_mut(target_stride).enumerate() {
        compose_row(
            source.as_raw(),
            &window,
            row,
            output_y,
            fill,
            fill_is_zero,
            &mut counts,
        )?;
    }
    image_from_raw(frame.width, frame.height, output)
        .map(|image| (image, counts.blocks, counts.tail))
}

/// Appends full-width native-byte content with the archived fill-row strategy.
pub(crate) fn pad_vertical_append<P, const VECTOR_FILL_ROW: bool>(
    source: &ImageBuffer<P, Vec<u8>>,
    frame: super::pad_dimensions::PadFrame,
    fill: [u8; 4],
) -> Result<Option<(ImageBuffer<P, Vec<u8>>, u64, u64)>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let Some(rows) = super::pad_dimensions::full_width_vertical_rows(source.dimensions(), frame)
    else {
        return Ok(None);
    };
    let channels = usize::from(P::CHANNEL_COUNT);
    if !(1..=4).contains(&channels) {
        return Err(PilError::InternalError(
            "Pad vector mode has invalid channels".into(),
        ));
    }
    let output_dims = CheckedDims::new_allow_empty(frame.width, frame.height, channels as u8)?;
    let output_len = output_dims.total_bytes();
    let target_stride = checked_product(
        usize::try_from(frame.width)
            .map_err(|_| PilError::DimensionError("Pad output width overflow".into()))?,
        channels,
        "Pad output row overflow",
    )?;
    let source_stride = checked_product(
        usize::try_from(source.width())
            .map_err(|_| PilError::DimensionError("Pad source width overflow".into()))?,
        channels,
        "Pad source row overflow",
    )?;
    let source_len = checked_product(
        source_stride,
        usize::try_from(source.height())
            .map_err(|_| PilError::DimensionError("Pad source height overflow".into()))?,
        "Pad source size overflow",
    )?;
    if source_len != source.as_raw().len() {
        return Err(PilError::InternalError(
            "Pad source buffer shape mismatch".into(),
        ));
    }

    let fill = &fill[..channels];
    let (fill_row, vector_blocks, scalar_tail) = if VECTOR_FILL_ROW {
        let mut row = Vec::new();
        row.try_reserve_exact(target_stride)
            .map_err(|error| PilError::MemoryError(error.to_string()))?;
        row.resize(target_stride, 0);
        let counts = fill_row(&mut row, fill);
        (row, counts.0, counts.1)
    } else {
        (fill.repeat(target_stride / channels), 0, 0)
    };
    if fill_row.len() != target_stride {
        return Err(PilError::InternalError(
            "Pad fill row shape mismatch".into(),
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for _ in 0..rows.top {
        output.extend_from_slice(&fill_row);
    }
    output.extend_from_slice(source.as_raw());
    for _ in 0..rows.bottom {
        output.extend_from_slice(&fill_row);
    }
    if output.len() != output_len {
        return Err(PilError::InternalError(
            "Pad output buffer shape mismatch".into(),
        ));
    }
    image_from_raw(frame.width, frame.height, output)
        .map(|image| Some((image, vector_blocks, scalar_tail)))
}

#[derive(Clone, Copy)]
struct ContentWindow {
    source_stride: usize,
    copy_height: usize,
    copy_bytes: usize,
    source_y_start: usize,
    destination_y_start: usize,
    source_x_start_bytes: usize,
    content_start: usize,
    content_end: usize,
}

impl ContentWindow {
    fn is_identity(
        self,
        source_width: usize,
        source_height: usize,
        frame: super::pad_dimensions::PadFrame,
        target_stride: usize,
    ) -> bool {
        self.source_y_start == 0
            && self.destination_y_start == 0
            && self.source_x_start_bytes == 0
            && self.content_start == 0
            && self.content_end == target_stride
            && source_width == frame.width as usize
            && source_height == frame.height as usize
            && self.copy_height == source_height
            && self.source_stride == target_stride
    }
}

#[derive(Default)]
struct VectorCounts {
    blocks: u64,
    tail: u64,
}

impl VectorCounts {
    fn add(&mut self, (blocks, tail): (u64, u64)) {
        self.blocks = self.blocks.saturating_add(blocks);
        self.tail = self.tail.saturating_add(tail);
    }
}

fn content_window(
    frame: super::pad_dimensions::PadFrame,
    channels: usize,
    target_stride: usize,
    source_stride: usize,
    source_width: usize,
    source_height: usize,
) -> Result<ContentWindow, PilError> {
    let x_window =
        super::pad_dimensions::clipped_axis(frame.offset_x, source_width, frame.width as usize);
    let y_window =
        super::pad_dimensions::clipped_axis(frame.offset_y, source_height, frame.height as usize);
    let copy_bytes = checked_product(x_window.length, channels, "Pad content row overflow")?;
    let source_x_start_bytes = checked_product(
        x_window.source_start,
        channels,
        "Pad source offset overflow",
    )?;
    let content_start = checked_product(
        x_window.destination_start,
        channels,
        "Pad content offset overflow",
    )?;
    let content_end = content_start
        .checked_add(copy_bytes)
        .ok_or_else(|| PilError::DimensionError("Pad content end overflow".into()))?;
    let source_x_end = x_window
        .source_start
        .checked_add(x_window.length)
        .ok_or_else(|| PilError::DimensionError("Pad horizontal extent overflow".into()))?;
    let destination_x_end = x_window
        .destination_start
        .checked_add(x_window.length)
        .ok_or_else(|| PilError::DimensionError("Pad horizontal extent overflow".into()))?;
    let source_y_end = y_window
        .source_start
        .checked_add(y_window.length)
        .ok_or_else(|| PilError::DimensionError("Pad vertical extent overflow".into()))?;
    let destination_y_end = y_window
        .destination_start
        .checked_add(y_window.length)
        .ok_or_else(|| PilError::DimensionError("Pad vertical extent overflow".into()))?;
    if destination_x_end > frame.width as usize
        || destination_y_end > frame.height as usize
        || source_x_end > source_width
        || source_y_end > source_height
        || content_end > target_stride
        || copy_bytes > source_stride
    {
        return Err(PilError::InternalError(
            "Pad content window exceeds its image".into(),
        ));
    }
    Ok(ContentWindow {
        source_stride,
        copy_height: if x_window.length == 0 {
            0
        } else {
            y_window.length
        },
        copy_bytes,
        source_y_start: y_window.source_start,
        destination_y_start: y_window.destination_start,
        source_x_start_bytes,
        content_start,
        content_end,
    })
}

fn compose_row(
    source: &[u8],
    window: &ContentWindow,
    row: &mut [u8],
    output_y: usize,
    fill: &[u8],
    fill_is_zero: bool,
    counts: &mut VectorCounts,
) -> Result<(), PilError> {
    if output_y < window.destination_y_start
        || output_y - window.destination_y_start >= window.copy_height
    {
        if !fill_is_zero {
            counts.add(fill_row(row, fill));
        }
        return Ok(());
    }
    if !fill_is_zero {
        counts.add(fill_row(&mut row[..window.content_start], fill));
    }
    let source_start = (window.source_y_start + output_y - window.destination_y_start)
        .checked_mul(window.source_stride)
        .and_then(|row| row.checked_add(window.source_x_start_bytes))
        .ok_or_else(|| PilError::DimensionError("Pad source row offset overflow".into()))?;
    let source_end = source_start
        .checked_add(window.copy_bytes)
        .ok_or_else(|| PilError::DimensionError("Pad source row end overflow".into()))?;
    counts.add(
        copy_native_bytes(
            &source[source_start..source_end],
            &mut row[window.content_start..window.content_end],
        )
        .ok_or_else(|| PilError::InternalError("Pad vector copy shape mismatch".into()))?,
    );
    if !fill_is_zero {
        counts.add(fill_row(&mut row[window.content_end..], fill));
    }
    Ok(())
}

fn fill_row(row: &mut [u8], fill: &[u8]) -> (u64, u64) {
    let vector_len = row.len() / 16 * 16;
    let mut blocks = 0u64;
    for start in (0..vector_len).step_by(16) {
        let block = u8x16::new(std::array::from_fn(|lane| {
            fill[(start + lane) % fill.len()]
        }));
        row[start..start + 16].copy_from_slice(&block.to_array());
        blocks = blocks.saturating_add(1);
    }
    let tail = row.len() - vector_len;
    if tail != 0 {
        let block = u8x16::new(std::array::from_fn(|lane| {
            fill[(vector_len + lane) % fill.len()]
        }));
        row[vector_len..].copy_from_slice(&block.to_array()[..tail]);
        blocks = blocks.saturating_add(1);
    }
    (blocks, tail as u64)
}

fn checked_product(left: usize, right: usize, message: &'static str) -> Result<usize, PilError> {
    left.checked_mul(right)
        .ok_or_else(|| PilError::DimensionError(message.into()))
}

fn image_from_raw<P>(
    width: u32,
    height: u32,
    raw: Vec<u8>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    ImageBuffer::from_raw(width, height, raw)
        .ok_or_else(|| PilError::InternalError("Pad output buffer shape mismatch".into()))
}
