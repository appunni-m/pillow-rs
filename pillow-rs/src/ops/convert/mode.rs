use crate::error::PilError;
use crate::raster::Mode;

/// Parses a Pillow mode string into a pipeline color mode.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `s` is not one of the modes supported
/// by core conversion.
pub fn parse_mode(s: &str) -> Result<Mode, PilError> {
    match s {
        "L" => Ok(Mode::ImageLuma8),
        "LA" => Ok(Mode::ImageLumaA8),
        "RGB" => Ok(Mode::ImageRgb8),
        "RGBA" => Ok(Mode::ImageRgba8),
        "CMYK" => Ok(Mode::ImageCmyk8),
        "YCbCr" => Ok(Mode::ImageYCbCr8),
        "HSV" => Ok(Mode::ImageHsv8),
        "I" => Ok(Mode::ImageI32),
        "F" => Ok(Mode::ImageF32),
        "P" => Ok(Mode::ImagePalette8),
        "1" => Ok(Mode::One),
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
