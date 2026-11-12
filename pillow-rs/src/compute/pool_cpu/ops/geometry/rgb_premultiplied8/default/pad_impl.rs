//! Native-sample default Pad kernel for ImageRgbPremultiplied8.

use crate::compute::pool_cpu::ops::geometry::helpers::{pad_dimensions::PadFrame, pad_samples as samples};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn pad(image: &RgbaImage, frame: PadFrame) -> Result<RgbaImage, PilError> {
    let fill = match frame.color {
        Some(color) => [color.0, color.1, color.2, color.3],
        None => [0, 0, 0, 0],
    };
    samples::pad(image, frame, fill)
}
