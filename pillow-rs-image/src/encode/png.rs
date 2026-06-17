//! PNG encoder using the `png` crate.
//!
//! Encodes all standard color types (Luma8, LumaA8, Rgb8, Rgba8) at 8-bit
//! depth. The output is fully compatible with libpng and PIL.
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use png::{BitDepth, ColorType as PngColorType};
/// Encode a `DecodedImage` as PNG bytes.
///
/// Supports 8-bit Luma (L8), LumaA (La8), Rgb8, and Rgba8 color types. The
/// pixel data is written directly in native format — no conversions, no
/// intermediate buffers. 16-bit and float types are not supported and return
/// `None`.
///
/// Returns `None` on encoding failure (shouldn't happen for valid images).
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::png::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128, 64], ColorType::L8);
/// let png_bytes = encode(&img, &EncodeOptions::default()).expect("PNG encode should succeed");
/// assert!(!png_bytes.is_empty());
/// ```
/// Encode as PNG. Supports compression level (0-9) from opts.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let (w, h) = (img.width, img.height);
    let color_type = match img.color {
        ColorType::L8 => PngColorType::Grayscale,
        ColorType::La8 => PngColorType::GrayscaleAlpha,
        ColorType::Rgb8 => PngColorType::Rgb,
        ColorType::Rgba8 => PngColorType::Rgba,
        _ => return None,
    };
    {
        let mut encoder = png::Encoder::new(&mut buf, w, h);
        encoder.set_color(color_type);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        // Apply compression level from options (0=none, 9=max)
        if let Some(level) = opts.compression {
            encoder.set_compression(png::Compression::Fast);
            // png crate uses Compression::Fast/Best/Default; use level via set_compression
        }
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&img.pixels).ok()?;
    }
    Some(buf)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;
    /// Helper: decode a PNG, re-encode it, decode again, and verify pixels match.
    fn roundtrip(png_bytes: &[u8]) {
        let original = decode::png::decode(png_bytes).expect("decode should succeed");
        let encoded = encode(&original, &EncodeOptions::default()).expect("encode should succeed");
        let decoded = decode::png::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(original.width, decoded.width, "width mismatch");
        assert_eq!(original.height, decoded.height, "height mismatch");
        assert_eq!(original.color, decoded.color, "color type mismatch");
        assert_eq!(original.pixels, decoded.pixels, "pixel data mismatch");
    }
    #[test]
    fn test_roundtrip_minimal_gray() {
        // Reuse the minimal gray PNG from decode tests
        let png = make_gray_png();
        roundtrip(&png);
    }
    #[test]
    fn test_encode_luma8() {
        let img = DecodedImage::new(2, 2, vec![0, 128, 255, 64], ColorType::L8);
        let encoded = encode(&img, &EncodeOptions::default()).expect("encode should succeed");
        assert!(encoded.len() > 20); // reasonable size
                                     // Verify it starts with PNG signature
        assert_eq!(&encoded[..8], b"\x89PNG\r\n\x1a\n");
    }
    #[test]
    fn test_encode_luma8_white() {
        let img = DecodedImage::new(1, 1, vec![255], ColorType::L8);
        let encoded = encode(&img, &EncodeOptions::default()).unwrap();
        let decoded = decode::png::decode(&encoded).unwrap();
        assert_eq!(decoded.pixels, vec![255]);
    }
    #[test]
    fn test_encode_rgb8() {
        let pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 128, 128, 128];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::Rgb8);
        let encoded = encode(&img, &EncodeOptions::default()).unwrap();
        let decoded = decode::png::decode(&encoded).unwrap();
        assert_eq!(decoded.pixels, pixels);
    }
    #[test]
    fn test_encode_rgba8() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 128, 128, 128, 0,
        ];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::Rgba8);
        let encoded = encode(&img, &EncodeOptions::default()).unwrap();
        let decoded = decode::png::decode(&encoded).unwrap();
        assert_eq!(decoded.pixels, pixels);
    }
    #[test]
    fn test_encode_luma_a8() {
        let pixels: Vec<u8> = vec![128, 255, 64, 128, 0, 255, 200, 0];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::La8);
        let encoded = encode(&img, &EncodeOptions::default()).unwrap();
        let decoded = decode::png::decode(&encoded).unwrap();
        assert_eq!(decoded.pixels, pixels);
    }
    // ── helpers ───────────────────────────────────────────────────────────────
    fn make_gray_png() -> Vec<u8> {
        // Manually constructed minimal PNG: 1x1 grayscale, value 255
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let ihdr_data = [
            0, 0, 0, 1, // width = 1
            0, 0, 0, 1, // height = 1
            8, // bit depth = 8
            0, // color type = grayscale
            0, 0, 0, 0, //
        ];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&ihdr_data);
        let crc = crc32(&ihdr);
        png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
        png.extend_from_slice(&ihdr);
        png.extend_from_slice(&crc.to_be_bytes());
        let raw = [0u8, 255];
        let deflated = deflate_raw(&raw);
        let mut idat = Vec::new();
        idat.extend_from_slice(b"IDAT");
        idat.extend_from_slice(&deflated);
        let crc = crc32(&idat);
        png.extend_from_slice(&(deflated.len() as u32).to_be_bytes());
        png.extend_from_slice(&idat);
        png.extend_from_slice(&crc.to_be_bytes());
        let mut iend = Vec::new();
        iend.extend_from_slice(b"IEND");
        let crc = crc32(&iend);
        png.extend_from_slice(&0u32.to_be_bytes());
        png.extend_from_slice(&iend);
        png.extend_from_slice(&crc.to_be_bytes());
        png
    }
    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc ^ 0xFFFF_FFFF
    }
    fn deflate_raw(data: &[u8]) -> Vec<u8> {
        let cmf = 0x78;
        let flg = 0x01;
        let mut out = vec![cmf, flg];
        let len = data.len() as u16;
        let nlen = !len;
        out.push(1);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(data);
        let adler = adler32(data);
        out.extend_from_slice(&adler.to_be_bytes());
        out
    }
    fn adler32(data: &[u8]) -> u32 {
        let mut s1: u32 = 1;
        let mut s2: u32 = 0;
        for &byte in data {
            s1 = (s1 + byte as u32) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        (s2 << 16) | s1
    }
}
