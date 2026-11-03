//! Full-width Pad composition using a prepared exact-mode Resize strategy.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::{self, PadFrame};
use crate::compute::pool_cpu::ops::resize::ops::PreparedResize;
use crate::error::PilError;
use crate::raster::{GrayImage, Mode};

pub(crate) fn resize_into_canvas(
    image: &GrayImage,
    frame: PadFrame,
    resize: &PreparedResize<'_>,
) -> Result<Option<GrayImage>, PilError> {
    if !resize.supports_pad_window()
        || frame.offset_x != 0
        || frame.offset_y < 0
        || frame.content_width != frame.width
        || frame.content_height >= frame.height
        || frame.offset_y > i64::from(frame.height - frame.content_height)
        || image.dimensions() == (frame.content_width, frame.content_height)
    {
        return Ok(None);
    }
    let canvas = CheckedDims::new(frame.width, frame.height, 1)?;
    let content = CheckedDims::new(frame.content_width, frame.content_height, 1)?;
    let offset_y = u32::try_from(frame.offset_y)
        .map_err(|_| PilError::DimensionError("Pad vertical offset overflow".into()))?;
    let window = dimensions::resize_window(canvas, content, offset_y, Mode::ImageLuma8)?;
    let fill = super::super::default::pad_impl::fill(frame.color);
    let mut output = vec![fill[0]; canvas.total_bytes()];
    if !resize.execute_into_pad_window(&mut output[window])? {
        return Ok(None);
    }
    GrayImage::from_raw(frame.width, frame.height, output)
        .map(Some)
        .ok_or_else(|| PilError::ValueError("pad_l: failed to create output buffer".into()))
}
