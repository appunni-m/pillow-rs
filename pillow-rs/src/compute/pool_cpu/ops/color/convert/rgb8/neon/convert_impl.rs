//! NEON Convert routes for native RGB8 input.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

/// Executes a prepared RGB8 NEON strategy.
///
/// # Safety
///
/// The caller must use a NEON architecture selected from this mode's support
/// declaration for the same target.
pub(crate) unsafe fn execute(
    source: &RgbImage,
    target: Mode,
) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 => {
            super::super::helpers::vector::luma_alpha(source, target)
        }
        Mode::ImageRgb8 => super::super::helpers::vector::identity(source),
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 => {
            // NO_PILLOW_SUPPORT: RGBa and RGBX are core-only targets; this
            // native expansion supplies opaque alpha or padding as required.
            // SAFETY: the strategy is selected only by the NEON support entry.
            let output = unsafe { super::rgba_impl::execute(source)? };
            super::super::helpers::vector::rgba_for_target(output, target)
        }
        Mode::ImageCmyk8 => super::super::helpers::vector::cmyk(source),
        Mode::ImageYCbCr8 => super::super::helpers::vector::ycbcr(source),
        Mode::ImageHsv8 => super::super::helpers::vector::hsv(source),
        Mode::ImageI32 => super::super::helpers::vector::i32(source),
        Mode::ImageF32 => super::super::helpers::vector::f32(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGB8 NEON target".into(),
        )),
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn luma8(source: &RgbImage) -> Result<DynamicImage, PilError> {
    use crate::compute::pool_cpu::ops::grayscale::rgb8 as grayscale_rgb8;

    let kernel = grayscale_rgb8::default::grayscale_impl::prepare(source)?;
    let pixels = kernel.dimensions.total_pixels();
    let (bytes, blocks) = grayscale_rgb8::neon::grayscale_impl::execute(
        kernel.input.as_raw(),
        pixels,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        (bytes, blocks, 0),
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
fn luma8(_source: &RgbImage) -> Result<DynamicImage, PilError> {
    Err(PilError::InternalError(
        "Convert selected an unavailable RGB8 NEON luma strategy".into(),
    ))
}
