//! Native-sample default Pad kernel for ImageRgba32F.
// NO_PILLOW_SUPPORT: internal high-depth native samples have no Pillow Pad mode identity.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::Rgba32FImage;

pub(crate) fn pad(image: &Rgba32FImage, frame: PadFrame) -> Result<Rgba32FImage, PilError> {
    let fill = match frame.native_color {
        Some(input) => [
            input.float_component(0, 0.0) as f32,
            input.float_component(1, 0.0) as f32,
            input.float_component(2, 0.0) as f32,
            input.float_component(3, 255.0) as f32,
        ],
        None => match frame.color {
            Some(color) => [
                color.0 as f32,
                color.1 as f32,
                color.2 as f32,
                color.3 as f32,
            ],
            None => [0.0; 4],
        },
    };
    samples::pad(image, frame, fill)
}
