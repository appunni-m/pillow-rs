//! Encode dispatcher — encode_format routes to per-format encoders.

use crate::encode_options::EncodeOptions;
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
pub fn encode_format(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => jpeg::encode(img, opts),
        ImageFormat::Png => png::encode(img, opts),
        ImageFormat::Gif => gif::encode(img, opts),
        ImageFormat::Bmp => bmp::encode(img, opts),
        ImageFormat::Tiff => tiff::encode(img, opts),
        ImageFormat::WebP => webp::encode(img, opts),
        ImageFormat::Ico => ico::encode(img, opts),
        ImageFormat::Avif => avif::encode(img, opts),
    }
}
