//! Native-sample default Pad kernel for ImageRgb32F.
// NO_PILLOW_SUPPORT: internal high-depth native samples have no Pillow Pad mode identity.

use crate::compute::pool_cpu::ops::geometry::helpers::{
    pad_dimensions::PadFrame, pad_samples as samples,
};
use crate::error::PilError;
use crate::raster::Rgb32FImage;

pub(crate) fn pad(image: &Rgb32FImage, frame: PadFrame) -> Result<Rgb32FImage, PilError> {
    let fill = match frame.native_color {
        Some(input) => [
            input.float_component(0, 0.0) as f32,
            input.float_component(1, 0.0) as f32,
            input.float_component(2, 0.0) as f32,
            0.0,
        ],
        None => match frame.color {
            Some(color) => [
                color.0 as f32,
                color.1 as f32,
                color.2 as f32,
                color.0 as f32,
            ],
            None => [0.0; 4],
        },
    };
    samples::pad(image, frame, fill)
}
