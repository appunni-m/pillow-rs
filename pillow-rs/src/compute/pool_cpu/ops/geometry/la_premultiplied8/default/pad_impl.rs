//! Native-sample default Pad kernel for ImageLaPremultiplied8.

use crate::compute::pool_cpu::ops::geometry::helpers::{
    pad_dimensions::PadFrame, pad_samples as samples,
};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn pad(image: &GrayAlphaImage, frame: PadFrame) -> Result<GrayAlphaImage, PilError> {
    let fill = match frame.color {
        Some(color) => [color.0, color.3, color.0, color.0],
        None => [0, 0, 0, 0],
    };
    samples::pad(image, frame, fill)
}
