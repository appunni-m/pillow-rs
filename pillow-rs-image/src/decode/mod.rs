//! Decode dispatcher — decode_format routes to per-format decoders.

use crate::types::{DecodedImage, ImageFormat};

pub mod avif;
pub mod bmp;
pub mod gif;
pub mod ico;
pub mod jpeg;
pub mod png;
pub mod tiff;
pub mod webp;

/// Dispatch decoding to the appropriate format-specific decoder.
pub fn decode_format(data: &[u8], format: ImageFormat) -> Option<DecodedImage> {
    match format {
        ImageFormat::Jpeg => jpeg::decode(data),
        ImageFormat::Png => png::decode(data),
        ImageFormat::Gif => gif::decode(data),
        ImageFormat::Bmp => bmp::decode(data),
        ImageFormat::Tiff => tiff::decode(data),
        ImageFormat::WebP => webp::decode(data),
        ImageFormat::Ico => ico::decode(data),
        ImageFormat::Avif => avif::decode(data),
    }
}
