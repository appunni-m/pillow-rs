//! Encode dispatcher — encode_format routes to per-format encoders.

use crate::encode_options::EncodeOptions;
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

/// Dispatch encoding to the appropriate format-specific encoder.
pub fn encode_format(
    img: &DecodedImage,
    format: ImageFormat,
    opts: &EncodeOptions,
) -> Option<Vec<u8>> {
    match format {
        ImageFormat::Jpeg => jpeg::encode(img, opts),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::encode(img, opts),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => None,
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::encode(img, opts),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => None,
        ImageFormat::Bmp => bmp::encode(img, opts),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::encode(img, opts),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => None,
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::encode(img, opts),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => None,
        ImageFormat::Ico => ico::encode(img, opts),
        ImageFormat::Avif => avif::encode(img, opts),
    }
}
