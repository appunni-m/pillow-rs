//! Native-sample default Pad kernel for ImageRgb32F.
// NO_PILLOW_SUPPORT: internal high-depth native samples have no Pillow Pad mode identity.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::Rgb32FImage;

pub(crate) fn pad(image: &Rgb32FImage, frame: PadFrame) -> Result<Rgb32FImage, PilError> {
    let fill = match frame.color {
        Some(color) => [color.0 as f32, color.1 as f32, color.2 as f32, color.0 as f32],
        None => [0 as f32, 0 as f32, 0 as f32, 0 as f32],
    };
    samples::pad(image, frame, fill)
}
