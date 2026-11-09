//! Default native Contrast implementation for ImageRgbx8.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn execute<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    base: &ContrastBase,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    crate::compute::pool_cpu::ops::enhance::helpers::contrast::execute_image(source, base, factor)
}
