//! JPEG encoder — baseline DCT sequential using the `jpeg-encoder` crate.
//!
//! Supports Rgb8 (RGB JPEG), L8 (Grayscale JPEG), Rgba8 (alpha stripped to RGB),
//! and La8 (alpha stripped to Grayscale). Quality defaults to 85.
//!
//! Output is a standard baseline JPEG readable by libjpeg, PIL, and our own
//! `decode::jpeg` decoder.

use crate::types::{ColorType, DecodedImage};

/// Encode a `DecodedImage` as JPEG bytes at quality 85.
///
/// Supported color types:
/// - `Rgb8` → RGB JPEG (3 bytes/pixel input)
/// - `L8`   → Grayscale JPEG (1 byte/pixel input)
/// - `Rgba8`→ RGBA input with alpha stripped, encoded as RGB JPEG
/// - `La8`  → LumaA input with alpha stripped, encoded as Grayscale JPEG
///
/// Unsupported types (L16, La16, Rgb16, Rgba16, Rgb32F, Rgba32F) return `None`.
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::jpeg::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128, 0, 255, 64], ColorType::Rgb8);
/// let jpeg = encode(&img).expect("JPEG encode should succeed");
/// assert!(!jpeg.is_empty());
/// ```
pub fn encode(img: &DecodedImage) -> Option<Vec<u8>> {
    let (w, h) = (img.width, img.height);
    let mut buf = Vec::new();

    match img.color {
        ColorType::Rgb8 => {
            let enc = jpeg_encoder::Encoder::new(&mut buf, 85);
            enc.encode(&img.pixels, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
                .ok()?;
        }
        ColorType::L8 => {
            let enc = jpeg_encoder::Encoder::new(&mut buf, 85);
            enc.encode(&img.pixels, w as u16, h as u16, jpeg_encoder::ColorType::Luma)
                .ok()?;
        }
        ColorType::Rgba8 => {
            // Strip alpha channel: 4 bytes/pixel → 3 bytes/pixel RGB
            let rgb: Vec<u8> = img.pixels.chunks_exact(4).flat_map(|c| c[0..3].iter().copied()).collect();
            let enc = jpeg_encoder::Encoder::new(&mut buf, 85);
            enc.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
                .ok()?;
        }
        ColorType::La8 => {
            // Strip alpha channel: 2 bytes/pixel → 1 byte/pixel Luma
            let gray: Vec<u8> = img.pixels.chunks_exact(2).map(|c| c[0]).collect();
            let enc = jpeg_encoder::Encoder::new(&mut buf, 85);
            enc.encode(&gray, w as u16, h as u16, jpeg_encoder::ColorType::Luma)
                .ok()?;
        }
        _ => return None,
    }

    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    /// Decode a JPEG → re-encode → check output is valid and decodable
    /// (not pixel-exact, since JPEG is lossy — we just verify valid output).
    fn valid_roundtrip(jpeg_bytes: &[u8]) {
        let original = decode::jpeg::decode(jpeg_bytes).expect("decode should succeed");
        let encoded = encode(&original).expect("encode should succeed");
        let decoded = decode::jpeg::decode(&encoded).expect("re-decode should succeed");

        assert_eq!(original.width, decoded.width, "width mismatch");
        assert_eq!(original.height, decoded.height, "height mismatch");
        assert_eq!(original.color, decoded.color, "color type mismatch");
        // JPEG is lossy — just check dimensions and color, not pixels
    }

    #[test]
    fn test_encode_rgb8() {
        let pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128];
        let img = DecodedImage::new(2, 2, pixels, ColorType::Rgb8);
        let encoded = encode(&img).expect("encode should succeed");
        assert!(encoded.len() > 20);
        // Verify JPEG SOI marker
        assert_eq!(&encoded[..2], &[0xFF, 0xD8]);
        // Verify EOI marker at end
        assert_eq!(&encoded[encoded.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn test_encode_l8_gray() {
        let pixels: Vec<u8> = vec![0, 128, 255, 64];
        let img = DecodedImage::new(2, 2, pixels, ColorType::L8);
        let encoded = encode(&img).expect("encode should succeed");
        assert!(encoded.len() > 20);
        assert_eq!(&encoded[..2], &[0xFF, 0xD8]);
        assert_eq!(&encoded[encoded.len() - 2..], &[0xFF, 0xD9]);
    }

    #[test]
    fn test_encode_rgba8_strips_alpha() {
        let pixels: Vec<u8> = vec![255, 0, 0, 128, 0, 255, 0, 64, 0, 0, 255, 32];
        let img = DecodedImage::new(1, 3, pixels, ColorType::Rgba8);
        let encoded = encode(&img).expect("encode should succeed");
        assert!(encoded.len() > 20);
        // Decode and verify it's RGB (no alpha)
        let decoded = decode::jpeg::decode(&encoded).expect("re-decode failed");
        assert_eq!(decoded.color, ColorType::Rgb8);
    }

    #[test]
    fn test_encode_la8_strips_alpha() {
        let pixels: Vec<u8> = vec![128, 255, 64, 128, 200, 0];
        let img = DecodedImage::new(1, 3, pixels, ColorType::La8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::jpeg::decode(&encoded).expect("re-decode failed");
        assert_eq!(decoded.color, ColorType::L8);
    }

    #[test]
    fn test_encode_unsupported_type_returns_none() {
        let img = DecodedImage::new(1, 1, vec![0; 6], ColorType::Rgb16);
        assert!(encode(&img).is_none());
    }

    #[test]
    fn test_valid_roundtrip_rgb() {
        let jpeg = make_test_jpeg_rgb();
        valid_roundtrip(&jpeg);
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn make_test_jpeg_rgb() -> Vec<u8> {
        // Create a minimal 2×2 RGB image and encode it as JPEG
        let pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128];
        let img = DecodedImage::new(2, 2, pixels, ColorType::Rgb8);
        encode(&img).unwrap()
    }
}
