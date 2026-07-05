//! BMP encoder — pure Rust, no external crate.
//!
//! Writes BITMAPFILEHEADER + BITMAPINFOHEADER + optional palette + pixel data.
//! Supports L8 (8-bit indexed with grayscale palette), Rgb8 (24-bit BGR),
//! and Rgba8 (32-bit BGRA). Rows are written bottom-up with 4-byte padding.
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
/// Row size in bytes (padded to 4-byte boundary), matching the decoder formula.
fn row_size(bits_per_pixel: u16, width: u32) -> usize {
    (((bits_per_pixel as u64) * (width as u64)).div_ceil(32) * 4) as usize
}
/// Encode a `DecodedImage` as BMP bytes.
///
/// Supports:
/// - `L8`: 8-bit with grayscale palette (256 entries)
/// - `Rgb8`: 24-bit BGR (R and B swapped)
/// - `Rgba8`: 32-bit BGRA
///
/// Returns `None` for unsupported color types.
pub fn encode(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width;
    let h = img.height;
    if w == 0 || h == 0 {
        return None;
    }
    // Validate pixel buffer size matches the expected w×h×channels layout.
    // The BMP decoder can return packed bits for 1-bit images as L8, which
    // would have a different buffer size. We reject those here.
    let expected_len = w as usize
        * h as usize
        * match img.color {
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
        data.push(0); // reserved
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
            data.push(pixels[offset]); // R
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
            data.push(pixels[offset]); // R
            data.push(pixels[offset + 3]); // A
        }
        // Pad row to 4-byte boundary (should be 0 for 32-bit, but just in case)
        for _ in 0..padding {
            data.push(0);
        }
    }
    Some(data)
}
