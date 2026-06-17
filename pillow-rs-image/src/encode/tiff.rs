//! TIFF encoder — wraps the `tiff` crate.
//!
//! Supports L8 (Grayscale 8-bit), Rgb8 (RGB 24-bit), and Rgba8 (RGBA 32-bit)
//! color types. Uses uncompressed encoding for maximum compatibility.

use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;
use tiff::encoder::{colortype, TiffEncoder};

/// Encode a `DecodedImage` as TIFF bytes.
///
/// Maps color types to TIFF photometric interpretations:
/// - `L8` → `Gray8` (BlackIsZero)
/// - `Rgb8` → `RGB8`
/// - `Rgba8` → `RGBA8`
///
/// Returns `None` for unsupported color types or zero-dimension images.
pub fn encode(img: &DecodedImage) -> Option<Vec<u8>> {
    let w = img.width;
    let h = img.height;
    if w == 0 || h == 0 {
        return None;
    }

    let mut buf = Vec::new();
    // Wrap in Cursor for Write+Seek required by TiffEncoder
    let mut cursor = Cursor::new(&mut buf);

    match img.color {
        ColorType::L8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::Gray8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        ColorType::Rgb8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::RGB8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        ColorType::Rgba8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::RGBA8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    fn roundtrip(tiff_bytes: &[u8]) {
        let original = decode::tiff::decode(tiff_bytes).expect("decode should succeed");
        let encoded = encode(&original).expect("encode should succeed");
        let decoded = decode::tiff::decode(&encoded).expect("re-decode should succeed");

        assert_eq!(original.width, decoded.width, "width mismatch");
        assert_eq!(original.height, decoded.height, "height mismatch");
        assert_eq!(original.color, decoded.color, "color type mismatch");
        assert_eq!(original.pixels, decoded.pixels, "pixel data mismatch");
    }

    #[test]
    fn test_encode_l8_roundtrip() {
        let tiff = build_gray_tiff();
        roundtrip(&tiff);
    }

    #[test]
    fn test_encode_rgb8_roundtrip() {
        let tiff = build_rgb_tiff();
        roundtrip(&tiff);
    }

    #[test]
    fn test_encode_l8_from_pixels() {
        let pixels: Vec<u8> = vec![0, 128, 200, 255];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::L8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::tiff::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn test_encode_rgb8_from_pixels() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128,
        ];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::Rgb8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::tiff::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn test_encode_rgba8_from_pixels() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 128, 128, 128, 0,
        ];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::Rgba8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::tiff::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn test_unsupported_color_type() {
        let img = DecodedImage::new(1, 1, vec![0, 0], ColorType::La8);
        assert!(encode(&img).is_none());
    }

    // ── helpers ───────────────────────────────────────────────────────────

    fn build_gray_tiff() -> Vec<u8> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        let mut tiff = TiffEncoder::new(&mut cursor).unwrap();
        tiff.write_image::<colortype::Gray8>(2, 2, &[0u8, 128, 200, 255])
            .unwrap();
        buf
    }

    fn build_rgb_tiff() -> Vec<u8> {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        let mut tiff = TiffEncoder::new(&mut cursor).unwrap();
        tiff.write_image::<colortype::RGB8>(
            2,
            2,
            &[
                255, 0, 0, // red
                0, 255, 0, // green
                0, 0, 255, // blue
                128, 128, 128, // gray
            ],
        )
        .unwrap();
        buf
    }
}
