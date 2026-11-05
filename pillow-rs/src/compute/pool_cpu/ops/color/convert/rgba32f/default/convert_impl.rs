//! Direct destination strategies from native RGBA32F samples.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Rgba};
use super::super::super::helpers::high_depth::HighDepthPixel;

type Source = ImageBuffer<Rgba<f32>, Vec<f32>>;

pub(crate) fn execute(source: &Source, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return super::super::super::helpers::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                let rgba = pixel.rgba8();
                [rgba[0], rgba[1], rgba[2]]
            }),
        );
    }
    if super::super::super::helpers::luma16::is_luma16_target(target) {
        return super::super::super::helpers::high_depth::execute_i16(source, target);
    }
    if target == Mode::ImageLuma8 {
        let kernel = crate::compute::pool_cpu::ops::grayscale::rgba32f::default::grayscale_impl::prepare(source)?;
        let output = crate::compute::pool_cpu::ops::grayscale::rgba32f::default::grayscale_impl::execute(kernel)?;
        return Ok(DynamicImage::ImageLuma8(output));
    }
    super::super::super::helpers::high_depth::execute(source, target)
}

pub(crate) fn execute_palette_target(
    source: &Source,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    super::super::super::helpers::high_depth::execute_palette_target(source, target, dither)
}
