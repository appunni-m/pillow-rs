//! Decode dispatcher — decode_format routes to per-format decoders.

use crate::types::{DecodedImage, ImageFormat};

pub mod avif;
pub mod bmp;
#[cfg(feature = "gif")]
pub mod gif;
pub mod ico;
pub mod jpeg;
#[cfg(feature = "png")]
pub mod png;
#[cfg(feature = "tiff")]
pub mod tiff;
#[cfg(feature = "webp")]
pub mod webp;

/// Dispatch decoding to the appropriate format-specific decoder.
pub fn decode_format(data: &[u8], format: ImageFormat) -> Option<DecodedImage> {
    match format {
        ImageFormat::Jpeg => jpeg::decode(data),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode(data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::decode(data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        ImageFormat::Bmp => bmp::decode(data),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::decode(data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode(data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        ImageFormat::Ico => ico::decode(data),
        ImageFormat::Avif => avif::decode(data),
    }
}
