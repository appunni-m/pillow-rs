//! Native-sample default Pad kernel for ImagePaletteAlpha8.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) const fn fill(color: Option<(u8, u8, u8, u8)>) -> [u8; 4] {
    match color {
        Some(color) => [color.0, color.3, 0, 0],
        None => [0, 0, 0, 0],
    }
}

pub(crate) fn pad(image: &GrayAlphaImage, frame: PadFrame) -> Result<GrayAlphaImage, PilError> {
    samples::pad(image, frame, fill(frame.color))
}
