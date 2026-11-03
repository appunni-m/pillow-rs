//! Full-width Pad composition using a prepared exact-mode Resize strategy.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::{self, PadFrame};
use crate::compute::pool_cpu::ops::resize::ops::PreparedResize;
use crate::error::PilError;
use crate::raster::{Mode, RgbaImage};

pub(crate) fn resize_into_canvas(
    image: &RgbaImage,
    frame: PadFrame,
    resize: &PreparedResize<'_>,
) -> Result<Option<RgbaImage>, PilError> {
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
    let canvas = CheckedDims::new(frame.width, frame.height, 4)?;
    let content = CheckedDims::new(frame.content_width, frame.content_height, 4)?;
    let offset_y = u32::try_from(frame.offset_y)
        .map_err(|_| PilError::DimensionError("Pad vertical offset overflow".into()))?;
    let window = dimensions::resize_window(canvas, content, offset_y, Mode::ImageRgbx8)?;
    let fill = super::super::default::pad_impl::fill(frame.color);
    let mut output = fill.repeat(canvas.width as usize);
    output = output.repeat(canvas.height as usize);
    if !resize.execute_into_pad_window(&mut output[window])? {
        return Ok(None);
    }
    RgbaImage::from_raw(frame.width, frame.height, output)
        .map(Some)
        .ok_or_else(|| PilError::ValueError("pad_rgbx: failed to create output buffer".into()))
}
