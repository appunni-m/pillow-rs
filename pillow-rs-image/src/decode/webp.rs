//! WebP decoder — pure Rust via image-webp (zero unsafe, `#![forbid(unsafe_code)]`).
//!
//! image-webp handles: lossy VP8, lossless VP8L, alpha (ALPH + VP8X),
//! animated (first frame), metadata (ICC/EXIF/XMP), and tiling.

use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

/// Decode a WebP image from raw bytes.
///
/// Returns `None` if the data is not valid WebP or if decoding fails.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let cursor = Cursor::new(data);

    let mut decoder = image_webp::WebPDecoder::new(cursor).ok()?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();

    let buf_size = decoder.output_buffer_size()?;
    let mut pixels = vec![0u8; buf_size];
    decoder.read_image(&mut pixels).ok()?;

    let color = if has_alpha {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    Some(DecodedImage::new(width, height, pixels, color))
}
