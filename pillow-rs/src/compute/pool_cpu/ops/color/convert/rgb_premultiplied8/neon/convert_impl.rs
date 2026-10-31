//! NEON-selected native RGBa Convert routes.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        _ => super::super::helpers::vector::execute(source, target),
    }
}

fn luma8(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::default::grayscale_impl::prepare(
        source,
    )?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgb_premultiplied8::neon::grayscale_impl::execute(
        kernel,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}
