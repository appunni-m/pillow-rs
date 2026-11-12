//! Native-sample default Pad kernel for ImageI16B.

use crate::compute::pool_cpu::ops::geometry::helpers::{
    pad_dimensions::PadFrame, pad_samples as samples,
};
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
        Some(input) => {
            let sample = input.component(0, 0) as u16;
            [sample; 4]
        }
        None => match frame.color {
            Some(color) => [color.0 as u16; 4],
            None => [0; 4],
        },
    };
    samples::pad(image, frame, fill)
}
