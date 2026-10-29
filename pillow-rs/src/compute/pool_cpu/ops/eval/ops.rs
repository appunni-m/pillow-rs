//! Eval mode support declaration and native-mode orchestration.

use crate::error::PilError;
use crate::image::Mode;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

#[path = "kernels/mod.rs"]
mod kernels;
pub(crate) use kernels::default::map_native_byte_lut;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[derive(Clone, Copy)]
enum ModeStrategy {
    Luma8,
    LumaAlpha8,
    Rgb8,
    Rgba8,
    Unsupported(&'static str),
}

/// Returns the complete Eval mode strategy table as an exhaustive enum match.
/// The selected byte strategy keeps each source in its existing carrier; modes
/// that do not have a byte-LUT contract report a stable rejection reason.
const fn mode_strategy(mode: Mode) -> ModeStrategy {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => ModeStrategy::Luma8,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            ModeStrategy::LumaAlpha8
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            ModeStrategy::Rgb8
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => ModeStrategy::Rgba8,
        Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            ModeStrategy::Unsupported("point operation not supported for this mode")
        }
    }
}

fn invalid_storage(mode: Mode) -> PilError {
    PilError::InternalError(format!(
        "Eval mode {mode:?} does not match its declared native storage"
    ))
}

fn eval_luma8(img: &DynamicImage, lut: &[u8], mode: Mode) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma8(gray) = img else {
        return Err(invalid_storage(mode));
    };
    let output = map_native_byte_lut::<1>(gray.as_raw(), lut)
        .ok_or_else(|| invalid_storage(mode))?;
    GrayImage::from_raw(gray.width(), gray.height(), output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| invalid_storage(mode))
}

fn eval_luma_alpha8(
    img: &DynamicImage,
    lut: &[u8],
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(la) = img else {
        return Err(invalid_storage(mode));
    };
    let output = map_native_byte_lut::<2>(la.as_raw(), lut)
        .ok_or_else(|| invalid_storage(mode))?;
    GrayAlphaImage::from_raw(la.width(), la.height(), output)
        .map(DynamicImage::ImageLumaA8)
        .ok_or_else(|| invalid_storage(mode))
}

fn eval_rgb8(img: &DynamicImage, lut: &[u8], mode: Mode) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb8(rgb) = img else {
        return Err(invalid_storage(mode));
    };
    let output = map_native_byte_lut::<3>(rgb.as_raw(), lut)
        .ok_or_else(|| invalid_storage(mode))?;
    RgbImage::from_raw(rgb.width(), rgb.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| invalid_storage(mode))
}

fn eval_rgba8(img: &DynamicImage, lut: &[u8], mode: Mode) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return Err(invalid_storage(mode));
    };
    let output = map_native_byte_lut::<4>(rgba.as_raw(), lut)
        .ok_or_else(|| invalid_storage(mode))?;
    RgbaImage::from_raw(rgba.width(), rgba.height(), output)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| invalid_storage(mode))
}

/// Applies the strategy selected by the wrapper's precomputed Pillow mode.
/// All supported implementations preserve the physical storage variant.
pub fn op_eval(img: &DynamicImage, lut: &[u8], mode: Mode) -> Result<DynamicImage, PilError> {
    match mode_strategy(mode) {
        ModeStrategy::Luma8 => eval_luma8(img, lut, mode),
        ModeStrategy::LumaAlpha8 => eval_luma_alpha8(img, lut, mode),
        ModeStrategy::Rgb8 => eval_rgb8(img, lut, mode),
        ModeStrategy::Rgba8 => eval_rgba8(img, lut, mode),
        ModeStrategy::Unsupported(reason) => Err(PilError::ValueError(reason.to_owned())),
    }
}
