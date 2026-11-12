//! SSSE3 RGB8-to-Luma8 Convert route.

use crate::compute::pool_cpu::ops::grayscale::rgb8 as grayscale_rgb8;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let kernel = grayscale_rgb8::default::grayscale_impl::prepare(source)?;
    let (bytes, blocks, tail) = grayscale_rgb8::ssse3::grayscale_impl::execute(kernel);
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        (bytes, blocks, tail),
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}
