//! JPEG encoder — baseline DCT sequential using the `jpeg-encoder` crate.
//!
//! Supports Rgb8 (RGB JPEG), L8 (Grayscale JPEG), Rgba8 (alpha stripped to RGB),
//! and La8 (alpha stripped to Grayscale). Quality defaults to 85.
//!
//! Output is a standard baseline JPEG readable by libjpeg, PIL, and our own
//! `decode::jpeg` decoder.

use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};

/// Encode a `DecodedImage` as JPEG bytes. Quality from options (default 85).
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (w, h) = (img.width, img.height);
    let quality = opts.quality.unwrap_or(85);
    let mut buf = Vec::new();

    match img.color {
        ColorType::Rgb8 => {
            let enc = jpeg_encoder::Encoder::new(&mut buf, quality);
            enc.encode(&img.pixels, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).ok()?;
        }
        ColorType::L8 => {
            let enc = jpeg_encoder::Encoder::new(&mut buf, quality);
            enc.encode(&img.pixels, w as u16, h as u16, jpeg_encoder::ColorType::Luma).ok()?;
        }
        ColorType::Rgba8 => {
            let rgb: Vec<u8> = img.pixels.chunks_exact(4).flat_map(|c| c[0..3].iter().copied()).collect();
            let enc = jpeg_encoder::Encoder::new(&mut buf, quality);
            enc.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).ok()?;
        }
        ColorType::La8 => {
            let gray: Vec<u8> = img.pixels.chunks_exact(2).map(|c| c[0]).collect();
            let enc = jpeg_encoder::Encoder::new(&mut buf, quality);
            enc.encode(&gray, w as u16, h as u16, jpeg_encoder::ColorType::Luma).ok()?;
        }
        _ => return None,
    }

    Some(buf)
}
