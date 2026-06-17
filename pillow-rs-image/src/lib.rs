//! pillow-rs-image — zero-dependency pixel-perfect image decoders and encoders.
//!
//! Goal: produce pixel-identical output to libjpeg/libpng so pillow-rs
//! parity tests pass. No external crates. Works on WASM.
//!
//! Architecture:
//!   &[u8] → decode() → DecodedImage { width, height, pixels, color }
//!   pillow-rs wraps DecodedImage into DynamicImage/Image::Loaded.

pub mod decode;
pub mod encode;
pub mod types;

pub use types::*;

/// Detect image format from magic bytes.
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() < 8 {
        return None;
    }
    if data[0] == 0xFF && data[1] == 0xD8 {
        return Some(ImageFormat::Jpeg);
    }
    if &data[0..8] == b"\x89PNG\r\n\x1a\n" {
        return Some(ImageFormat::Png);
    }
    if &data[0..4] == b"GIF8" {
        return Some(ImageFormat::Gif);
    }
    if &data[0..2] == b"BM" {
        return Some(ImageFormat::Bmp);
    }
    if data.len() >= 12 && &data[8..12] == b"WEBP" {
        return Some(ImageFormat::WebP);
    }
    if &data[0..4] == b"II\x2a\x00" || &data[0..4] == b"MM\x00\x2a" {
        return Some(ImageFormat::Tiff);
    }
    if &data[0..4] == b"\x00\x00\x01\x00" {
        return Some(ImageFormat::Ico);
    }
    if data.len() >= 12 && &data[4..12] == b"ftypavif" {
        return Some(ImageFormat::Avif);
    }
    None
}

/// Auto-detect format and decode image data.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let format = detect_format(data)?;
    decode::decode_format(data, format)
}

/// Encode a DecodedImage into the specified format.
pub fn encode(img: &DecodedImage, format: ImageFormat) -> Option<Vec<u8>> {
    encode::encode_format(img, format)
}
