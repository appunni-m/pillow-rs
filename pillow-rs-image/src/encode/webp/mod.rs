//! WebP encoder — lossless via image-webp, lossy via own VP8 pipeline.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

pub mod vp8;

/// Encode a DecodedImage to WebP format.
///
/// Lossless: uses `image-webp` VP8L encoder.
/// Lossy: uses our own pure-Rust VP8 intra-frame encoder.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    if opts.lossless == Some(true) {
        encode_lossless(img, opts)
    } else {
        encode_lossy(img, opts)
    }
}

/// Lossless VP8L encoding via image-webp `WebPEncoder`.
fn encode_lossless(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (width, height) = (img.width, img.height);
    let color = match img.color {
        ColorType::Rgb8 => image_webp::ColorType::Rgb8,
        ColorType::Rgba8 => image_webp::ColorType::Rgba8,
        _ => return None,
    };

    let mut out = Cursor::new(Vec::new());
    let encoder = image_webp::WebPEncoder::new(&mut out);
    encoder
        .encode(&img.pixels, width, height, color)
        .ok()?;

    Some(out.into_inner())
}

/// Lossy VP8 encoding — own pure-Rust implementation.
///
/// Encodes VP8 keyframe bitstream in RIFF/WEBP container.
fn encode_lossy(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let quality = opts.quality.unwrap_or(80).min(100);
    let encoded = vp8::encoder::encode_vp8_lossy(&img.pixels, img.width, img.height, quality as u8);
    Some(encoded)
}
