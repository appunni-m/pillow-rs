//! Native-sample default Pad kernel for ImageLuma8.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::{self, PadFrame}, samples};
use crate::error::PilError;
use crate::raster::{GrayImage, Mode};

pub(crate) const fn fill(color: Option<(u8, u8, u8, u8)>) -> [u8; 4] {
    match color {
        Some(color) => [color.0, 0, 0, 0],
        None => [0, 0, 0, 0],
    }
}

pub(crate) fn pad(image: &GrayImage, frame: PadFrame) -> Result<GrayImage, PilError> {
    samples::pad(image, frame, fill(frame.color))
}

pub(crate) fn resize_into_canvas(
    image: &GrayImage,
    frame: PadFrame,
    resize: &crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'_>,
) -> Result<Option<GrayImage>, PilError> {
    if !resize.supports_pad_window()
        || frame.content_width != frame.width
        || frame.content_height >= frame.height
        || image.dimensions() == (frame.content_width, frame.content_height)
    {
        return Ok(None);
    }
    let canvas = CheckedDims::new(frame.width, frame.height, 1)?;
    let content = CheckedDims::new(frame.content_width, frame.content_height, 1)?;
    let window = dimensions::resize_window(canvas, content, frame.offset_y, Mode::ImageLuma8)?;
    let mut output = vec![fill(frame.color)[0]; canvas.total_bytes()];
    if !resize.execute_into_pad_window(&mut output[window])? {
        return Ok(None);
    }
    GrayImage::from_raw(frame.width, frame.height, output)
        .map(Some)
        .ok_or_else(|| PilError::ValueError("pad_l: failed to create output buffer".into()))
}
