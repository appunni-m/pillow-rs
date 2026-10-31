//! Native-sample default Pad kernel for ImageRgbx8.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::{self, PadFrame}, samples};
use crate::error::PilError;
use crate::raster::{Mode, RgbaImage};

pub(crate) const fn fill(color: Option<(u8, u8, u8, u8)>) -> [u8; 4] {
    match color {
        Some(color) => [color.0, color.1, color.2, color.3],
        None => [0, 0, 0, 0],
    }
}

pub(crate) fn pad(image: &RgbaImage, frame: PadFrame) -> Result<RgbaImage, PilError> {
    samples::pad(image, frame, fill(frame.color))
}

pub(crate) fn resize_into_canvas(
    image: &RgbaImage,
    frame: PadFrame,
    resize: &crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'_>,
) -> Result<Option<RgbaImage>, PilError> {
    if !resize.supports_pad_window()
        || frame.content_width != frame.width
        || frame.content_height >= frame.height
        || image.dimensions() == (frame.content_width, frame.content_height)
    {
        return Ok(None);
    }
    let canvas = CheckedDims::new(frame.width, frame.height, 4)?;
    let content = CheckedDims::new(frame.content_width, frame.content_height, 4)?;
    let window = dimensions::resize_window(canvas, content, frame.offset_y, Mode::ImageRgbx8)?;
    let mut output = fill(frame.color).repeat(canvas.width as usize);
    output = output.repeat(canvas.height as usize);
    if !resize.execute_into_pad_window(&mut output[window])? {
        return Ok(None);
    }
    RgbaImage::from_raw(frame.width, frame.height, output)
        .map(Some)
        .ok_or_else(|| PilError::ValueError("pad_rgbx: failed to create output buffer".into()))
}
