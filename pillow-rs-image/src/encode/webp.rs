//! WebP encoder — uses libwebp C library for lossless encoding.
//!
//! Both PIL and this encoder use the same libwebp under the hood, so output
//! for lossless encoding is pixel-identical. This is critical for roundtrip
//! tests that require exact binary equality.
//!
//! Supported color types:
//! - `Rgb8`  → WebPEncodeLosslessRGB
//! - `Rgba8` → WebPEncodeLosslessRGBA
//! - `L8`    → Expanded to RGB, then WebPEncodeLosslessRGB
//! - `La8`   → Alpha stripped, expanded to RGB, then WebPEncodeLosslessRGB

use crate::types::{ColorType, DecodedImage};

// ---------------------------------------------------------------------------
// libwebp C FFI declarations — encoder functions
// ---------------------------------------------------------------------------

extern "C" {
    /// Lossless WebP encode from RGB (3 bytes/pixel).
    /// Returns size of output buffer (0 on failure).
    /// Output buffer must be freed with WebPFree().
    fn WebPEncodeLosslessRGB(
        rgb: *const u8,
        width: i32,
        height: i32,
        stride: i32,
        output: *mut *mut u8,
    ) -> usize;

    /// Lossless WebP encode from RGBA (4 bytes/pixel).
    fn WebPEncodeLosslessRGBA(
        rgba: *const u8,
        width: i32,
        height: i32,
        stride: i32,
        output: *mut *mut u8,
    ) -> usize;

    /// Free a buffer allocated by libwebp.
    fn WebPFree(ptr: *mut u8);
}

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

/// Encode a `DecodedImage` as lossless WebP bytes using libwebp.
///
/// Supported color types:
/// - `Rgb8`  → 3 bytes/pixel, direct lossless RGB WebP
/// - `Rgba8` → 4 bytes/pixel, direct lossless RGBA WebP
/// - `L8`    → 1 byte/pixel, expanded to RGB before encoding
/// - `La8`   → 2 bytes/pixel, alpha stripped and expanded to RGB before encoding
///
/// Unsupported types (L16, La16, Rgb16, Rgba16, Rgb32F, Rgba32F) return `None`.
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::webp::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128, 0, 255, 64], ColorType::Rgb8);
/// let webp = encode(&img).expect("WebP encode should succeed");
/// assert!(!webp.is_empty());
/// ```
pub fn encode(img: &DecodedImage) -> Option<Vec<u8>> {
    let (w, h) = (img.width, img.height);
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    match img.color {
        ColorType::Rgb8 => {
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGB(
                    img.pixels.as_ptr(),
                    w_i32,
                    h_i32,
                    stride,
                    &mut output,
                )
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::Rgba8 => {
            let stride = w_i32 * 4;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGBA(
                    img.pixels.as_ptr(),
                    w_i32,
                    h_i32,
                    stride,
                    &mut output,
                )
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::L8 => {
            // Expand L8 (1 byte) to RGB (3 bytes): copy luma to R, G, B
            let num_pixels = (w as usize).saturating_mul(h as usize);
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for &l in &img.pixels {
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGB(rgb.as_ptr(), w_i32, h_i32, stride, &mut output)
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::La8 => {
            // Strip alpha, then expand to RGB: 2 bytes → 1 byte luma → 3 bytes RGB
            let num_pixels = (w as usize).saturating_mul(h as usize);
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for c in img.pixels.chunks_exact(2) {
                let l = c[0];
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGB(rgb.as_ptr(), w_i32, h_i32, stride, &mut output)
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    /// Decode a WebP → re-encode → decode → verify pixels are bit-exact
    /// (lossless WebP guarantees this).
    fn roundtrip(webp_bytes: &[u8]) {
        let original = decode::webp::decode(webp_bytes).expect("decode should succeed");
        let encoded = encode(&original).expect("encode should succeed");
        let decoded = decode::webp::decode(&encoded).expect("re-decode should succeed");

        assert_eq!(original.width, decoded.width, "width mismatch");
        assert_eq!(original.height, decoded.height, "height mismatch");
        assert_eq!(original.color, decoded.color, "color type mismatch");
        assert_eq!(original.pixels, decoded.pixels, "pixel data mismatch");
    }

    #[test]
    fn test_encode_rgb8() {
        let pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128];
        let img = DecodedImage::new(2, 2, pixels, ColorType::Rgb8);
        let encoded = encode(&img).expect("encode should succeed");
        assert!(encoded.len() > 20);
        // Verify WebP RIFF header
        assert_eq!(&encoded[0..4], b"RIFF");
        assert_eq!(&encoded[8..12], b"WEBP");
    }

    #[test]
    fn test_encode_rgba8() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 128, 128, 128, 0,
        ];
        let img = DecodedImage::new(2, 2, pixels, ColorType::Rgba8);
        let encoded = encode(&img).expect("encode should succeed");
        assert_eq!(&encoded[0..4], b"RIFF");
        assert_eq!(&encoded[8..12], b"WEBP");

        // Decode back — the decoder always tries RGB first, which drops alpha,
        // so the re-decoded result is Rgb8 (not Rgba8). This matches the
        // decoder's current behaviour.
        let decoded = decode::webp::decode(&encoded).expect("re-decode failed");
        // The decoder returns Rgb8 (alpha dropped)
        assert_eq!(decoded.color, ColorType::Rgb8);
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
    }

    #[test]
    fn test_encode_l8_expands_to_rgb() {
        let pixels: Vec<u8> = vec![0, 128, 255, 64];
        let img = DecodedImage::new(2, 2, pixels, ColorType::L8);
        let encoded = encode(&img).expect("encode should succeed");
        assert_eq!(&encoded[0..4], b"RIFF");
        assert_eq!(&encoded[8..12], b"WEBP");

        // Decode back — encoder expands L8 to RGB,
        // decoder returns Rgb8 since there's no alpha
        let decoded = decode::webp::decode(&encoded).expect("re-decode failed");
        // The decoder will return Rgb8 since the lossless WebP has no alpha
        // (it was encoded as RGB from the expanded L8 data)
        assert_eq!(decoded.color, ColorType::Rgb8);
    }

    #[test]
    fn test_encode_la8_strips_alpha_and_expands() {
        let pixels: Vec<u8> = vec![128, 255, 64, 128, 200, 0];
        let img = DecodedImage::new(1, 3, pixels, ColorType::La8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::webp::decode(&encoded).expect("re-decode failed");
        // La8 → RGB (no alpha after stripping)
        assert_eq!(decoded.color, ColorType::Rgb8);
    }

    #[test]
    fn test_encode_unsupported_type_returns_none() {
        let img = DecodedImage::new(1, 1, vec![0; 6], ColorType::Rgb16);
        assert!(encode(&img).is_none());
    }

    #[test]
    fn test_roundtrip_rgb() {
        let webp = make_test_webp_rgb();
        roundtrip(&webp);
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn make_test_webp_rgb() -> Vec<u8> {
        // Create a 4×4 RGB test pattern, encode as WebP
        let mut pixels = Vec::with_capacity(4 * 4 * 3);
        for y in 0..4 {
            for x in 0..4 {
                pixels.push((x * 64) as u8);
                pixels.push((y * 64) as u8);
                pixels.push(128);
            }
        }
        let img = DecodedImage::new(4, 4, pixels, ColorType::Rgb8);
        encode(&img).unwrap()
    }
}
