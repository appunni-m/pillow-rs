//! Native-sample default Pad kernel for ImageRgbx8.

use crate::compute::pool_cpu::ops::geometry::helpers::{
    pad_dimensions::PadFrame, pad_samples as samples,
};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) const fn fill(color: Option<(u8, u8, u8, u8)>) -> [u8; 4] {
    match color {
        Some(color) => [color.0, color.1, color.2, color.3],
        None => [0, 0, 0, 0],
    }
}

pub(crate) fn pad(image: &RgbaImage, frame: PadFrame) -> Result<RgbaImage, PilError> {
    samples::pad(image, frame, fill(frame.color))
}
