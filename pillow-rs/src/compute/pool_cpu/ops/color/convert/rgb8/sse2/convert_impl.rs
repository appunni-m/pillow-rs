//! SSE2 Convert routes for native RGB8 input.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        Mode::ImageLumaA8 => super::super::helpers::vector::luma_alpha(source, target),
        // NO_PILLOW_SUPPORT: RGB to La is an explicitly core-only target.
        Mode::ImageLaPremultiplied8 => {
            super::super::helpers::vector::luma_alpha(source, target)
        }
        Mode::ImageRgb8 => super::super::helpers::vector::identity(source),
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 => {
            // NO_PILLOW_SUPPORT: RGBa and RGBX are core-only targets; this
            // native expansion supplies opaque alpha or padding as required.
            let output = super::super::helpers::rgba::execute(source)?;
            super::super::helpers::vector::rgba_for_target(output, target)
        }
        Mode::ImageCmyk8 => super::super::helpers::vector::cmyk(source),
        Mode::ImageYCbCr8 => super::super::helpers::vector::ycbcr(source),
        Mode::ImageHsv8 => super::super::helpers::vector::hsv(source),
        Mode::ImageI32 => super::super::helpers::vector::i32(source),
        Mode::ImageF32 => super::super::helpers::vector::f32(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGB8 SSE2 target".into(),
        )),
    }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_feature = "ssse3"),
    not(target_feature = "avx2")
))]
fn luma8(source: &RgbImage) -> Result<DynamicImage, PilError> {
    use crate::compute::pool_cpu::ops::grayscale::rgb8 as grayscale_rgb8;

    let kernel = grayscale_rgb8::default::grayscale_impl::prepare(source)?;
    let (bytes, blocks, tail) = grayscale_rgb8::sse2::grayscale_impl::execute(kernel);
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        (bytes, blocks, tail),
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

#[cfg(not(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_feature = "ssse3"),
    not(target_feature = "avx2")
)))]
fn luma8(_source: &RgbImage) -> Result<DynamicImage, PilError> {
    Err(PilError::InternalError(
        "Convert selected an unavailable RGB8 SSE2 luma strategy".into(),
    ))
}
