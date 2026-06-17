//! TIFF decoder — handles both little-endian (II) and big-endian (MM) byte orders.
//!
//! Supported color types:
//!   - Gray(8)   → Luma8
//!   - Gray(16)  → Luma8 (high-byte downscale)
//!   - RGB(8)    → Rgb8
//!   - RGBA(8)   → Rgba8
//!   - CMYK(8)   → Rgb8
//!
//! Compression handled by the `tiff` crate: None, LZW, Deflate, PackBits, etc.
//! Byte order handled transparently by the `tiff` crate.

use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

/// Decode TIFF bytes into a DecodedImage.
///
/// Returns `None` for unsupported color types or corrupt data.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(data)).ok()?;
    let (w, h) = decoder.dimensions().ok()?;
    let color_type = decoder.colortype().ok()?;

    let result = decoder.read_image().ok()?;

    match color_type {
        tiff::ColorType::Gray(8) => {
            if let tiff::decoder::DecodingResult::U8(img) = result {
                Some(DecodedImage::new(w, h, img, ColorType::L8))
            } else {
                None
            }
        }
        tiff::ColorType::Gray(16) => {
            if let tiff::decoder::DecodingResult::U16(img) = result {
                // Downscale 16-bit to 8-bit by taking the high byte.
                let img8: Vec<u8> = img.into_iter().map(|v| (v >> 8) as u8).collect();
                Some(DecodedImage::new(w, h, img8, ColorType::L8))
            } else {
                None
            }
        }
        tiff::ColorType::RGB(8) => {
            if let tiff::decoder::DecodingResult::U8(img) = result {
                Some(DecodedImage::new(w, h, img, ColorType::Rgb8))
            } else {
                None
            }
        }
        tiff::ColorType::RGBA(8) => {
            if let tiff::decoder::DecodingResult::U8(img) = result {
                Some(DecodedImage::new(w, h, img, ColorType::Rgba8))
            } else {
                None
            }
        }
        tiff::ColorType::CMYK(8) => {
            if let tiff::decoder::DecodingResult::U8(img) = result {
                // Inverse CMYK → RGB: remove the key plate, invert the remaining channels.
                // Uses the standard formula:
                //   R = (255 - C) * (255 - K) / 255
                //   G = (255 - M) * (255 - K) / 255
                //   B = (255 - Y) * (255 - K) / 255
                let rgb: Vec<u8> = img
                    .chunks_exact(4)
                    .flat_map(|cmyk| {
                        let c = cmyk[0] as u32;
                        let m = cmyk[1] as u32;
                        let y = cmyk[2] as u32;
                        let k = cmyk[3] as u32;
                        let r = (255 - c) * (255 - k) / 255;
                        let g = (255 - m) * (255 - k) / 255;
                        let b = (255 - y) * (255 - k) / 255;
                        [r as u8, g as u8, b as u8]
                    })
                    .collect();
                Some(DecodedImage::new(w, h, rgb, ColorType::Rgb8))
            } else {
                None
            }
        }
        _ => None,
    }
}
