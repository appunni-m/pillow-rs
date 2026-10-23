use crate::error::PilError;
use crate::pipeline::ColorMode;

/// Parses a Pillow mode string into a pipeline color mode.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `s` is not one of the modes supported
/// by core conversion.
pub fn parse_mode(s: &str) -> Result<ColorMode, PilError> {
    match s {
        "L" => Ok(ColorMode::L),
        "LA" => Ok(ColorMode::LA),
        "RGB" => Ok(ColorMode::RGB),
        "RGBA" => Ok(ColorMode::RGBA),
        "CMYK" => Ok(ColorMode::CMYK),
        "YCbCr" => Ok(ColorMode::YCbCr),
        "HSV" => Ok(ColorMode::HSV),
        "I" => Ok(ColorMode::I),
        "F" => Ok(ColorMode::F),
        "P" => Ok(ColorMode::P),
        "1" => Ok(ColorMode::Mode1),
        _ => Err(PilError::ValueError(format!("Unknown mode: {}", s))),
    }
}

pub(super) fn is_luma16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

/// Modes that require special handling when converting FROM them.
/// These modes store pixel data in a non-standard interpretation within
/// standard DynamicImage variants (e.g., CMYK values stored as RGBA).
pub(super) fn is_nonstandard_mode(mode: &str) -> bool {
    matches!(mode, "CMYK" | "HSV" | "YCbCr" | "I" | "F" | "P" | "PA")
}

/// Extracts the Y band of a YCbCr image as luma bytes.
///
/// Pillow's C converter (`src/libImaging/Convert.c`) maps YCbCr to L and "1"
/// through the Y channel directly rather than through the RGB luminance.
pub(super) fn ycbcr_luma8(img: &crate::raster::DynamicImage) -> crate::raster::GrayImage {
    let rgb = img.to_rgb8();
    crate::raster::GrayImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        crate::raster::Luma([rgb.get_pixel(x, y)[0]])
    })
}
