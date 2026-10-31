//! Native-sample default Pad kernel for ImageI16L.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, samples};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn pad<P>(image: &ImageBuffer<P, Vec<u16>>, frame: PadFrame) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let fill = match frame.color {
        Some(color) => [color.0 as u16, color.0 as u16, color.0 as u16, color.0 as u16],
        None => [0, 0, 0, 0],
    };
    samples::pad(image, frame, fill)
}
