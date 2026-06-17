//! JPEG encoder — baseline DCT sequential.
//!
//! Converts all input color types to Rgb8, then encodes using a pure-Rust
//! baseline JPEG encoder. Currently a stub — returns None.
//!
//! Future implementation will use `jpeg-encoder` crate or a custom
//! IJG-compatible encoder for pixel-perfect libjpeg parity.

use crate::types::{ColorType, DecodedImage};

/// Encode a `DecodedImage` as JPEG bytes.
///
/// JPEG does not support alpha natively, so RGBA and LA images are flattened
/// to RGB / L (alpha is discarded). Luma images are expanded to RGB.
///
/// Returns `None` — JPEG encoding not yet implemented.
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::jpeg::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128], ColorType::Rgb8);
/// assert!(encode(&img).is_none()); // stub for now
/// ```
pub fn encode(img: &DecodedImage) -> Option<Vec<u8>> {
    let (w, h) = (img.width, img.height);
    let num_pixels = (w as usize).saturating_mul(h as usize);

    // Convert to RGB before encoding (JPEG doesn't natively support alpha).
    // For 16-bit and float types, return None (unsupported).
    let _rgb: Vec<u8> = match img.color {
        ColorType::Rgb8 => {
            // Direct passthrough — already in RGB format
            return None; // TODO: implement direct RGB encode
        }
        ColorType::Rgba8 => {
            // Strip alpha: 4-byte → 3-byte
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for c in img.pixels.chunks_exact(4) {
                rgb.push(c[0]);
                rgb.push(c[1]);
                rgb.push(c[2]);
            }
            rgb
        }
        ColorType::L8 => {
            // Expand grayscale to RGB: 1-byte → 3-byte (same value in R, G, B)
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for &l in &img.pixels {
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            rgb
        }
        ColorType::La8 => {
            // Strip alpha, expand to RGB: 2-byte → 3-byte
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for c in img.pixels.chunks_exact(2) {
                let l = c[0];
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            rgb
        }
        _ => return None,
    };

    let _buf: Vec<u8> = Vec::new();
    // TODO: Use jpeg-encoder crate or simple baseline JPEG encoder
    // let mut encoder = jpeg_encoder::Encoder::new(&mut buf, 90);
    // encoder.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).ok()?;
    let _ = (w, h);
    None // stub — encoder not yet implemented
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jpeg_encode_stub() {
        let img = DecodedImage::new(1, 1, vec![255, 0, 0], ColorType::Rgb8);
        assert!(encode(&img).is_none());
    }
}
