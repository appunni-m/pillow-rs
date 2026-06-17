//! BMP encoder — pure Rust, no external crate.
//!
//! Writes BITMAPFILEHEADER + BITMAPINFOHEADER + optional palette + pixel data.
//! Supports L8 (8-bit indexed with grayscale palette), Rgb8 (24-bit BGR),
//! and Rgba8 (32-bit BGRA). Rows are written bottom-up with 4-byte padding.

use crate::types::{ColorType, DecodedImage};

/// Row size in bytes (padded to 4-byte boundary), matching the decoder formula.
fn row_size(bits_per_pixel: u16, width: u32) -> usize {
    (((bits_per_pixel as u64) * (width as u64) + 31) / 32 * 4) as usize
}

/// Encode a `DecodedImage` as BMP bytes.
///
/// Supports:
/// - `L8`: 8-bit with grayscale palette (256 entries)
/// - `Rgb8`: 24-bit BGR (R and B swapped)
/// - `Rgba8`: 32-bit BGRA
///
/// Returns `None` for unsupported color types.
pub fn encode(img: &DecodedImage) -> Option<Vec<u8>> {
    let w = img.width;
    let h = img.height;
    if w == 0 || h == 0 {
        return None;
    }
    // Validate pixel buffer size matches the expected w×h×channels layout.
    // The BMP decoder can return packed bits for 1-bit images as L8, which
    // would have a different buffer size. We reject those here.
    let expected_len = w as usize * h as usize * match img.color {
        ColorType::L8 => 1,
        ColorType::Rgb8 => 3,
        ColorType::Rgba8 => 4,
        _ => 0,
    };
    if img.pixels.len() != expected_len {
        return None;
    }

    match img.color {
        ColorType::L8 => encode_l8(w, h, &img.pixels),
        ColorType::Rgb8 => encode_rgb24(w, h, &img.pixels),
        ColorType::Rgba8 => encode_rgb32(w, h, &img.pixels),
        _ => None,
    }
}

/// Encode an 8-bit indexed BMP with grayscale palette.
fn encode_l8(w: u32, h: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let bits_per_pixel = 8u16;
    let row_len = row_size(bits_per_pixel, w);
    let pixel_bytes_per_row = w as usize;
    let padding = row_len - pixel_bytes_per_row;
    let palette_size = 256 * 4; // 256 entries × 4 bytes (B, G, R, reserved)
    let pixel_data_offset = 14u32 + 40 + palette_size as u32;

    let mut data = Vec::with_capacity(pixel_data_offset as usize + row_len * h as usize);

    // --- BITMAPFILEHEADER (14 bytes) ---
    data.extend_from_slice(b"BM");
    let pixel_area = row_len * h as usize;
    let file_size = pixel_data_offset as usize + pixel_area;
    data.extend_from_slice(&(file_size as u32).to_le_bytes()); // bfSize
    data.extend_from_slice(&[0u8; 4]); // bfReserved1 + bfReserved2
    data.extend_from_slice(&pixel_data_offset.to_le_bytes()); // bfOffBits

    // --- BITMAPINFOHEADER (40 bytes) ---
    data.extend_from_slice(&40u32.to_le_bytes()); // biSize
    data.extend_from_slice(&(w as i32).to_le_bytes()); // biWidth
    data.extend_from_slice(&(h as i32).to_le_bytes()); // biHeight (bottom-up)
    data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    data.extend_from_slice(&bits_per_pixel.to_le_bytes()); // biBitCount
    data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    data.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage (0 for BI_RGB)
    data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed (0 = max for bpp)
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // --- Palette (256 entries, 4 bytes each: B, G, R, reserved) ---
    for i in 0u16..256 {
        let val = i as u8;
        data.push(val); // B
        data.push(val); // G
        data.push(val); // R
        data.push(0);   // reserved
    }

    // --- Pixel data (bottom-up) ---
    for y in (0..h as usize).rev() {
        let row_start = y * w as usize;
        let row_end = row_start + w as usize;
        data.extend_from_slice(&pixels[row_start..row_end]);
        // Pad row to 4-byte boundary
        for _ in 0..padding {
            data.push(0);
        }
    }

    Some(data)
}

/// Encode a 24-bit BMP from RGB8 pixels (BGR order, bottom-up).
fn encode_rgb24(w: u32, h: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let bits_per_pixel = 24u16;
    let row_len = row_size(bits_per_pixel, w);
    let pixel_bytes_per_row = w as usize * 3;
    let padding = row_len - pixel_bytes_per_row;
    let pixel_data_offset = 14u32 + 40;

    let mut data = Vec::with_capacity(pixel_data_offset as usize + row_len * h as usize);

    // --- BITMAPFILEHEADER (14 bytes) ---
    data.extend_from_slice(b"BM");
    let pixel_area = row_len * h as usize;
    let file_size = pixel_data_offset as usize + pixel_area;
    data.extend_from_slice(&(file_size as u32).to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&pixel_data_offset.to_le_bytes());

    // --- BITMAPINFOHEADER (40 bytes) ---
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(w as i32).to_le_bytes());
    data.extend_from_slice(&(h as i32).to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&bits_per_pixel.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    // --- Pixel data (bottom-up, BGR) ---
    for y in (0..h as usize).rev() {
        let row_start = y * w as usize * 3;
        for col in 0..w as usize {
            let offset = row_start + col * 3;
            // Swap R and B for BGR order
            data.push(pixels[offset + 2]); // B
            data.push(pixels[offset + 1]); // G
            data.push(pixels[offset + 0]); // R
        }
        // Pad row to 4-byte boundary
        for _ in 0..padding {
            data.push(0);
        }
    }

    Some(data)
}

/// Encode a 32-bit BMP from RGBA8 pixels (BGRA order, bottom-up).
fn encode_rgb32(w: u32, h: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let bits_per_pixel = 32u16;
    let row_len = row_size(bits_per_pixel, w);
    // For 32-bit, row_len should already equal w * 4, but we compute it for safety.
    let pixel_bytes_per_row = w as usize * 4;
    let padding = row_len - pixel_bytes_per_row;
    let pixel_data_offset = 14u32 + 40;

    let mut data = Vec::with_capacity(pixel_data_offset as usize + row_len * h as usize);

    // --- BITMAPFILEHEADER (14 bytes) ---
    data.extend_from_slice(b"BM");
    let pixel_area = row_len * h as usize;
    let file_size = pixel_data_offset as usize + pixel_area;
    data.extend_from_slice(&(file_size as u32).to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&pixel_data_offset.to_le_bytes());

    // --- BITMAPINFOHEADER (40 bytes) ---
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(w as i32).to_le_bytes());
    data.extend_from_slice(&(h as i32).to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&bits_per_pixel.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    // --- Pixel data (bottom-up, BGRA) ---
    for y in (0..h as usize).rev() {
        let row_start = y * w as usize * 4;
        for col in 0..w as usize {
            let offset = row_start + col * 4;
            // RGBA → BGRA
            data.push(pixels[offset + 2]); // B
            data.push(pixels[offset + 1]); // G
            data.push(pixels[offset + 0]); // R
            data.push(pixels[offset + 3]); // A
        }
        // Pad row to 4-byte boundary (should be 0 for 32-bit, but just in case)
        for _ in 0..padding {
            data.push(0);
        }
    }

    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode;

    fn roundtrip(bmp_bytes: &[u8]) {
        let original = decode::bmp::decode(bmp_bytes).expect("decode should succeed");
        let encoded = encode(&original).expect("encode should succeed");
        let decoded = decode::bmp::decode(&encoded).expect("re-decode should succeed");

        assert_eq!(original.width, decoded.width, "width mismatch");
        assert_eq!(original.height, decoded.height, "height mismatch");
        assert_eq!(original.color, decoded.color, "color type mismatch");
        assert_eq!(original.pixels, decoded.pixels, "pixel data mismatch");
    }

    #[test]
    fn test_encode_l8_roundtrip() {
        // Build a simple BMP manually: 2x2 grayscale
        let bmp = build_gray_bmp_8bit();
        roundtrip(&bmp);
    }

    #[test]
    fn test_encode_rgb24_roundtrip() {
        // Build a simple BMP manually: 2x2 RGB
        let bmp = build_rgb_bmp_24bit();
        roundtrip(&bmp);
    }

    #[test]
    fn test_encode_l8_from_pixels() {
        let pixels: Vec<u8> = vec![0, 128, 200, 255];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::L8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::bmp::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn test_encode_rgb8_from_pixels() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, // red
            0, 255, 0, // green
            0, 0, 255, // blue
            128, 128, 128, // gray
        ];
        let img = DecodedImage::new(2, 2, pixels.clone(), ColorType::Rgb8);
        let encoded = encode(&img).expect("encode should succeed");
        let decoded = decode::bmp::decode(&encoded).expect("re-decode should succeed");
        assert_eq!(decoded.pixels, pixels);
    }

    #[test]
    fn test_encode_rgba8_from_pixels() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, // red opaque
            0, 255, 0, 128, // green half-alpha
            0, 0, 255, 64, // blue low alpha
            128, 128, 128, 0, // gray transparent
        ];
        let img = DecodedImage::new(2, 2, pixels, ColorType::Rgba8);
        let encoded = encode(&img).expect("encode should succeed");
        // 32-bit BMP decoder strips alpha, so round-trip gives Rgb8
        let decoded = decode::bmp::decode(&encoded).expect("re-decode should succeed");
        // Check the RGB values match (alpha stripped)
        assert_eq!(decoded.color, ColorType::Rgb8);
    }

    #[test]
    fn test_unsupported_color_type() {
        let img = DecodedImage::new(1, 1, vec![0], ColorType::La8);
        assert!(encode(&img).is_none());
    }

    // ── helpers ───────────────────────────────────────────────────────────

    fn build_gray_bmp_8bit() -> Vec<u8> {
        let w = 2u32;
        let h = 2u32;
        let row_len = ((8 * w + 31) / 32 * 4) as usize; // = 4
        let palette_size = 256 * 4;
        let data_offset = 14u32 + 40 + palette_size as u32;
        let file_size = data_offset as usize + row_len * h as usize;

        let mut data = Vec::new();
        data.extend_from_slice(b"BM");
        data.extend_from_slice(&(file_size as u32).to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&data_offset.to_le_bytes());
        data.extend_from_slice(&40u32.to_le_bytes());
        data.extend_from_slice(&(w as i32).to_le_bytes());
        data.extend_from_slice(&(h as i32).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        // Palette: grayscale (B=G=R=i, reserved=0)
        for i in 0u16..256 {
            let v = i as u8;
            data.push(v); data.push(v); data.push(v); data.push(0);
        }

        // Pixel data (bottom-up): top-left=0, top-right=128, bottom-left=200, bottom-right=255
        // Row 1 (bottom in file, top of image): indices 0, 128
        data.push(0);
        data.push(128);
        data.push(0); data.push(0); // padding (4-2=2)
        // Row 0 (top in file, bottom of image): indices 200, 255
        data.push(200);
        data.push(255);
        data.push(0); data.push(0); // padding
        data
    }

    fn build_rgb_bmp_24bit() -> Vec<u8> {
        let w = 2u32;
        let h = 2u32;
        let row_len = ((24 * w + 31) / 32 * 4) as usize; // = 8 for w=2: (48+31)/32=2, *4=8
        let data_offset = 14u32 + 40;
        let file_size = data_offset as usize + row_len * h as usize;

        let mut data = Vec::new();
        data.extend_from_slice(b"BM");
        data.extend_from_slice(&(file_size as u32).to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&data_offset.to_le_bytes());
        data.extend_from_slice(&40u32.to_le_bytes());
        data.extend_from_slice(&(w as i32).to_le_bytes());
        data.extend_from_slice(&(h as i32).to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&24u16.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes());

        // Pixel data (bottom-up, BGR)
        // Bottom row (y=0 in file): B=0,G=0,R=255 (red), B=0,G=255,R=0 (green)
        data.push(0); data.push(0); data.push(255); // red (BGR)
        data.push(0); data.push(255); data.push(0); // green (BGR)
        data.push(0); data.push(0); // padding (8-6=2)
        // Top row (y=1 in file): B=255,G=0,R=0 (blue), B=128,G=128,R=128 (gray)
        data.push(255); data.push(0); data.push(0); // blue (BGR)
        data.push(128); data.push(128); data.push(128); // gray (BGR)
        data.push(0); data.push(0); // padding
        data
    }
}
