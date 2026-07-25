//! Image format parsing.
//!
//! This module maps Pillow-style format names such as `"PNG"` and `"JPEG"` to
//! `image-slash-star` codec identifiers. File paths and extension extraction
//! belong in binding crates; core receives the format string directly.

use image_slash_star::ImageFormat;

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
    ImageFormat::from_name(s)
        .map_err(|_| PilError::UnknownFormat(format!("Unsupported format: {s}")))
}
