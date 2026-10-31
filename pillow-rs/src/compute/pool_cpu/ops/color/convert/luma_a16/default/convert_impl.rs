//! Direct destination strategies from native luma-alpha 16-bit samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, LumaA, Mode};

type Source = ImageBuffer<LumaA<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Source, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLuma8 {
        let kernel = crate::compute::pool_cpu::ops::grayscale::luma_a16::default::grayscale_impl::prepare(source)?;
        let output = crate::compute::pool_cpu::ops::grayscale::luma_a16::default::grayscale_impl::execute(kernel)?;
        return Ok(DynamicImage::ImageLuma8(output));
    }
    super::super::super::helpers::high_depth::execute(source, target)
}
