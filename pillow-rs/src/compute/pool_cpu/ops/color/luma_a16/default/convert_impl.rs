//! Direct destination strategies from native luma-alpha 16-bit samples.

use crate::compute::pool_cpu::ops::color::helpers::convert::high_depth::HighDepthPixel;
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, ImageBuffer, LumaA, Mode, Rgba, RgbaImage};

type Source = ImageBuffer<LumaA<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Source, target: Mode) -> Result<DynamicImage, PilError> {
    if target == Mode::ImageLab8 {
        return crate::compute::pool_cpu::ops::color::helpers::convert::lab_target::execute(
            source.width(),
            source.height(),
            source.pixels().map(|pixel| {
                let rgba = pixel.rgba8();
                [rgba[0], rgba[1], rgba[2]]
            }),
        );
    }
    if target == Mode::ImageRgbx8 {
        return Ok(DynamicImage::ImageRgbx8(rgbx(source)));
    }
    if crate::compute::pool_cpu::ops::color::helpers::convert::luma16::is_luma16_target(target) {
        return crate::compute::pool_cpu::ops::color::helpers::convert::high_depth::execute_i16(source, target);
    }
    if target == Mode::ImageLuma8 {
        let kernel =
            crate::compute::pool_cpu::ops::grayscale::luma_a16::default::grayscale_impl::prepare(
                source,
            )?;
        let output =
            crate::compute::pool_cpu::ops::grayscale::luma_a16::default::grayscale_impl::execute(
                kernel,
            )?;
        return Ok(DynamicImage::ImageLuma8(output));
    }
    crate::compute::pool_cpu::ops::color::helpers::convert::high_depth::execute(source, target)
}

fn rgbx(source: &Source) -> RgbaImage {
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let LumaA([luma, alpha]) = pixel.luma_alpha8();
        *destination = Rgba([luma, luma, luma, alpha]);
    }
    output
}

pub(crate) fn execute_palette_target(
    source: &Source,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    crate::compute::pool_cpu::ops::color::helpers::convert::high_depth::execute_palette_target(source, target, dither)
}
