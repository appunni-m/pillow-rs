//! Image format parsing and detection.
//! Maps between Pillow format strings and pillow_rs_image::ImageFormat.

use pillow_rs_image::ImageFormat;

use crate::error::PilError;

/// Parse a format string (e.g. "PNG", "JPEG") into ImageFormat.
/// Case-insensitive. Supports: JPEG, PNG, GIF, BMP, TIFF, WEBP, ICO.
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
