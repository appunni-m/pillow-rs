//! Native-sample default Pad kernel for ImageLab8.

use crate::compute::pool_cpu::ops::geometry::helpers::{pad_dimensions::PadFrame, pad_samples as samples};
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn pad(image: &RgbImage, frame: PadFrame) -> Result<RgbImage, PilError> {
    let fill = match frame.color {
        Some(color) => [color.0, color.1, color.2, color.0],
        None => [0, 0, 0, 0],
    };
    samples::pad(image, frame, fill)
}
