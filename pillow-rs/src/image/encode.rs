//! Pillow-compatible image encoding and format validation.

use image_slash_star::{DecodedImage, ImageFormat, ImageMode, ImagePalette};

use crate::error::PilError;
use crate::format::parse_format_str;

use super::{Image, operational_palette};

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

impl Image {
    /// Encodes the image as PNG bytes.
    ///
    /// Paletted images are rendered through their palette before encoding so
    /// saved output contains visible RGB colors rather than raw index bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization or PNG encoding fails.
    pub fn to_png_bytes(&self) -> Result<Vec<u8>, PilError> {
        let decoded = self.decoded_for_encoding()?;
        Ok(image_slash_star::encode_default(
            &decoded,
            ImageFormat::Png,
        )?)
    }

    fn decoded_for_encoding(&self) -> Result<DecodedImage, PilError> {
        if let Image::Paletted(data) = self {
            let mut palette_bytes = operational_palette(data);
            if palette_bytes.len() < 768 {
                // The underlying indexed encoder requires a full 256-entry
                // palette; Pillow synthesizes missing entries at save time.
                palette_bytes.resize(768, 0);
            }
            let palette = ImagePalette::new(palette_bytes, data.palette_alpha.clone())?;
            return Ok(DecodedImage::with_mode(
                data.indices.width(),
                data.indices.height(),
                data.indices.as_bytes().to_vec(),
                ImageMode::P8,
            )
            .with_palette(palette));
        }
        if self.has_palette_mode() {
            let indices = self.materialize_indices()?.to_luma8();
            let mut palette = self.extract_palette().ok_or_else(|| {
                PilError::PaletteError("P-mode pipeline has no retained palette".to_owned())
            })?;
            if palette.len() < 768 {
                palette.resize(768, 0);
            }
            let alpha = self.palette_alpha().unwrap_or_default();
            return Ok(DecodedImage::with_mode(
                indices.width(),
                indices.height(),
                indices.into_raw(),
                ImageMode::P8,
            )
            .with_palette(ImagePalette::new(palette, alpha)?));
        }
        Ok(self.materialize()?.into_decoded())
    }
}
