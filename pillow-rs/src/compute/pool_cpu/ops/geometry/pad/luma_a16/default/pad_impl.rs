//! Native-sample default Pad kernel for ImageLumaA16.
// NO_PILLOW_SUPPORT: internal high-depth native samples have no Pillow Pad mode identity.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn pad<P>(
    image: &ImageBuffer<P, Vec<u16>>,
    frame: PadFrame,
) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let fill = match frame.native_color {
        Some(input) => [
            input.component(0, 0) as u16,
            input.component(1, 0) as u16,
            0,
            0,
        ],
        None => match frame.color {
            Some(color) => [
                color.0 as u16,
                color.3 as u16,
                color.0 as u16,
                color.0 as u16,
            ],
            None => [0; 4],
        },
    };
    samples::pad(image, frame, fill)
}
