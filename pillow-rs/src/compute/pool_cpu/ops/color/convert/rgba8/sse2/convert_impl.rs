//! SSE2-selected direct kernels for straight RGBA8 Convert.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageLumaA8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32 => super::super::helpers::vector::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBA SSE2 target".into(),
        )),
    }
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_feature = "ssse3"),
    not(target_feature = "avx2")
))]
fn luma8(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl::prepare(
        source,
    )?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgba8::sse2::grayscale_impl::execute(
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

#[cfg(not(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_feature = "ssse3"),
    not(target_feature = "avx2")
)))]
fn luma8(_source: &RgbaImage) -> Result<DynamicImage, PilError> {
    Err(PilError::InternalError(
        "Convert selected an unavailable RGBA SSE2 luma strategy".into(),
    ))
}
