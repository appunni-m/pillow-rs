//! Image encoder dispatch with Pillow-compatible format validation.

use super::Image;
use crate::error::PilError;
use crate::format::parse_format_str;

impl Image {
    /// Encodes the image using the requested Pillow format name.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format is unsupported or image
    /// materialization or encoding fails.
    pub fn encode(&self, format: &str) -> Result<Vec<u8>, PilError> {
        self.encode_with_options(format, &[])
    }

    /// Encodes the image with strict, format-specific encoder options.
    ///
    /// `options` uses the selected codec's legacy string-pair names, which
    /// are translated to typed encoder settings before dispatch. An empty
    /// option list follows the same default path as [`Image::encode`].
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the format, image mode, option name, option
    /// value, materialization, or encoding is unsupported.
    pub fn encode_with_options(
        &self,
        format: &str,
        options: &[(String, String)],
    ) -> Result<Vec<u8>, PilError> {
        let save_format = parse_format_str(format)?;
        let upper_format = format.to_ascii_uppercase();
        let mode = self.mode()?;
        // Pillow's encoders reject unsupported mode/format combinations with
        // an OSError naming both; replicate the public messages exactly.
        let rejection = match upper_format.as_str() {
            "PNG" if mode == "F" => Some("cannot write mode F as PNG"),
            "PNG" if mode == "CMYK" => Some("cannot write mode CMYK as PNG"),
            "BMP" if mode == "I" => Some("cannot write mode I as BMP"),
            "BMP" if mode == "F" => Some("cannot write mode F as BMP"),
            "BMP" if mode == "CMYK" => Some("cannot write mode CMYK as BMP"),
            _ => None,
        };
        if let Some(message) = rejection {
            return Err(PilError::OsError(message.into()));
        }
        if options.is_empty() {
            if upper_format == "PNG" {
                return self.to_png_bytes();
            }
            let decoded = self.decoded_for_encoding()?;
            return Ok(image_slash_star::encode_default(&decoded, save_format)?);
        }

        let options = image_slash_star::EncodeOptions::try_from_legacy_pairs(save_format, options)?;
        let decoded = self.decoded_for_encoding()?;
        Ok(image_slash_star::encode(&decoded, save_format, &options)?)
    }
}
