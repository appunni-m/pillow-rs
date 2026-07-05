//! Image format parsing.
//!
//! This module maps Pillow-style format names such as `"PNG"` and `"JPEG"` to
//! `pillow-rs-image` codec identifiers. File paths and extension extraction
//! belong in binding crates; core receives the format string directly.

use pillow_rs_image::ImageFormat;

use crate::error::PilError;

/// Parses a Pillow format string into an [`ImageFormat`].
///
/// Accepted names are case-insensitive. `"JPG"` is treated as `"JPEG"` and
/// `"TIF"` is treated as `"TIFF"`.
///
/// # Errors
///
/// Returns [`PilError::UnknownFormat`] when `s` is not a supported format name.
pub fn parse_format_str(s: &str) -> Result<ImageFormat, PilError> {
    match s.to_uppercase().as_str() {
        "JPEG" | "JPG" => Ok(ImageFormat::Jpeg),
        "PNG" => Ok(ImageFormat::Png),
        "GIF" => Ok(ImageFormat::Gif),
        "BMP" => Ok(ImageFormat::Bmp),
        "TIFF" | "TIF" => Ok(ImageFormat::Tiff),
        "WEBP" => Ok(ImageFormat::WebP),
        "ICO" => Ok(ImageFormat::Ico),
        _ => Err(PilError::UnknownFormat(format!(
            "Unsupported format: {}",
            s
        ))),
    }
}
