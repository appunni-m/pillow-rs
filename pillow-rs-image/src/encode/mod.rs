//! Encode dispatcher — encode_format routes to per-format encoders.

use crate::types::{DecodedImage, ImageFormat};

pub mod avif;
pub mod bmp;
pub mod gif;
pub mod ico;
pub mod jpeg;
pub mod png;
pub mod tiff;
pub mod webp;

/// Dispatch encoding to the appropriate format-specific encoder.
pub fn encode_format(img: &DecodedImage, format: ImageFormat) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => jpeg::encode(img),
        ImageFormat::Png => png::encode(img),
        ImageFormat::Gif => gif::encode(img),
        ImageFormat::Bmp => bmp::encode(img),
        ImageFormat::Tiff => tiff::encode(img),
        ImageFormat::WebP => webp::encode(img),
        ImageFormat::Ico => ico::encode(img),
        ImageFormat::Avif => avif::encode(img),
    }
}
