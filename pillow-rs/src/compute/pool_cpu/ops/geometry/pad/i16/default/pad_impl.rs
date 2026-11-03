//! Native-sample default Pad kernel for ImageI16.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn pad<P>(image: &ImageBuffer<P, Vec<u16>>, frame: PadFrame) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let fill = match frame.native_color {
        Some(input) => {
            // Pillow's I;16 `Image.new` fill stores the scalar's low 16 bits.
            // The cast wraps directly into this native sample representation.
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
