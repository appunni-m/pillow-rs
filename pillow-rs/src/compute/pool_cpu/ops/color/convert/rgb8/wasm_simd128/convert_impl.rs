//! WebAssembly SIMD128 Convert routes for exact native RGB8 input.

use super::super::helpers::vector;
use crate::compute::pool_cpu::ops::grayscale::rgb8 as grayscale_rgb8;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        Mode::ImageLumaA8 => vector::luma_alpha(source, target),
        // NO_PILLOW_SUPPORT: RGB to La is an explicitly core-only target.
        Mode::ImageLaPremultiplied8 => vector::luma_alpha(source, target),
        Mode::ImageRgb8 => vector::identity(source),
        Mode::ImageRgba8 => rgba(source, target),
        // NO_PILLOW_SUPPORT: RGBa and RGBX are core-only targets.
        Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 => rgba(source, target),
        Mode::ImageCmyk8 => vector::cmyk(source),
        Mode::ImageYCbCr8 => vector::ycbcr(source),
        Mode::ImageHsv8 => vector::hsv(source),
        Mode::ImageI32 => vector::i32(source),
        Mode::ImageF32 => vector::f32(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGB8 Wasm SIMD128 target".into(),
        )),
    }
}

fn rgba(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    let output = super::super::helpers::rgba::execute(source)?;
    vector::rgba_for_target(output, target)
}

fn luma8(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let kernel = grayscale_rgb8::default::grayscale_impl::prepare(source)?;
    let (bytes, blocks, tail) = grayscale_rgb8::wasm_simd128::grayscale_impl::execute(kernel);
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        (bytes, blocks, tail),
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}
