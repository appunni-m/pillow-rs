//! Direct destination strategies from native RGBA32F samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Rgba};

type Source = ImageBuffer<Rgba<f32>, Vec<f32>>;

pub(crate) fn execute(source: &Source, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLuma8 {
        let kernel = crate::compute::pool_cpu::ops::grayscale::rgba32f::default::grayscale_impl::prepare(source)?;
        let output = crate::compute::pool_cpu::ops::grayscale::rgba32f::default::grayscale_impl::execute(kernel)?;
        return Ok(DynamicImage::ImageLuma8(output));
    }
    super::super::super::helpers::high_depth::execute(source, target)
}
