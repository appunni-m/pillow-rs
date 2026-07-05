//! ICO encoder — wraps a BMP DIB inside an ICO container.
//!
//! ICO (Icon) files store one or more images. This encoder writes a single
//! 32-bit BGRA entry, wrapping the pixel data in a BITMAPINFOHEADER + AND
//! mask. Supports RGBA8 (4 bytes/pixel), RGB8 (converts to RGBA), and L8
//! (converts to RGBA).
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
/// Encode a `DecodedImage` as an ICO file (single 32-bit BGRA entry).
///
/// The output is a valid ICO container with one directory entry pointing to
/// BMP/DIB data:
/// - ICO header (6 bytes)
/// - Directory entry (16 bytes)
/// - BITMAPINFOHEADER (40 bytes)
/// - BGRA pixel data (bottom-up)
/// - AND mask (all zeros, for full opacity)
///
/// Supported input color types: `Rgba8`, `Rgb8`, `L8`. Other types return
/// `None`.
pub fn encode(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width as usize;
    let h = img.height as usize;
    if w == 0 || h == 0 || w > 256 || h > 256 {
        return None;
    }
    // Convert pixel data to BGRA (we always write 32-bit ICO entries)
    let bgra = match img.color {
        ColorType::Rgba8 => convert_rgba_to_bgra(&img.pixels),
        ColorType::Rgb8 => convert_rgb_to_bgra(&img.pixels),
        ColorType::L8 => convert_l8_to_bgra(&img.pixels),
        _ => return None,
    };
    // ICO header + directory entry size
    let header_size = 6;
    let dir_entry_size = 16;
    let data_offset = header_size + dir_entry_size;
    // BMP BITMAPINFOHEADER (40 bytes)
    let dib_header_size = 40u32;
    // Pixel data: each row is 4 bytes per pixel, bottom-up
    let row_bytes = w * 4;
    let pixel_data_size = row_bytes * h;
    // AND mask: 1 bit per pixel, each row padded to 4-byte boundary
    let and_mask_row_bytes = w.div_ceil(32) * 4;
    let and_mask_size = and_mask_row_bytes * h;
    // ICO BMP data: DIB header + pixels + AND mask
    let bmp_data_size = dib_header_size as usize + pixel_data_size + and_mask_size;
    let total_size = data_offset + bmp_data_size;
    let mut data = Vec::with_capacity(total_size);
    // --- ICO header (6 bytes) ---
    data.extend_from_slice(&[0u8; 2]); // reserved
    data.extend_from_slice(&1u16.to_le_bytes()); // type = ICO (1)
    data.extend_from_slice(&1u16.to_le_bytes()); // count = 1
    // --- Directory entry (16 bytes) ---
    // Width/height: 0 means 256; otherwise actual value
    if w == 256 {
        data.push(0);
    } else {
        data.push(w as u8);
    }
    if h == 256 {
        data.push(0);
    } else {
        data.push(h as u8);
    }
    data.push(0); // colors (0 = >= 256)
    data.push(0); // reserved
    data.extend_from_slice(&1u16.to_le_bytes()); // color planes
    data.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    data.extend_from_slice(&(bmp_data_size as u32).to_le_bytes()); // size of BMP data
    data.extend_from_slice(&(data_offset as u32).to_le_bytes()); // offset of BMP data
    // --- BMP data: BITMAPINFOHEADER (40 bytes) ---
    data.extend_from_slice(&dib_header_size.to_le_bytes()); // biSize
    data.extend_from_slice(&(w as u32).to_le_bytes()); // biWidth
    // ICO convention: height is doubled to include AND mask rows
    data.extend_from_slice(&((h as u32) * 2).to_le_bytes()); // biHeight
    data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    data.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    data.extend_from_slice(&0u32.to_le_bytes()); // biSizeImage
    data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
    // --- Pixel data (bottom-up BGRA) ---
    for y in (0..h).rev() {
        let row_start = y * row_bytes;
        data.extend_from_slice(&bgra[row_start..row_start + row_bytes]);
    }
    // --- AND mask (all zeros = fully opaque) ---
    for _ in 0..h {
        for _ in 0..and_mask_row_bytes {
            data.push(0);
        }
    }
    Some(data)
}
/// Convert RGBA8 pixels (R,G,B,A) to BGRA (B,G,R,A).
fn convert_rgba_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(pixels.len());
    for chunk in pixels.chunks_exact(4) {
        bgra.push(chunk[2]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[0]); // R
        bgra.push(chunk[3]); // A
    }
    bgra
}
/// Convert RGB8 pixels (R,G,B) to BGRA (B,G,R,A=255).
fn convert_rgb_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let pixel_count = pixels.len() / 3;
    let mut bgra = Vec::with_capacity(pixel_count * 4);
    for chunk in pixels.chunks_exact(3) {
        bgra.push(chunk[2]); // B
        bgra.push(chunk[1]); // G
        bgra.push(chunk[0]); // R
        bgra.push(255); // A (fully opaque)
    }
    bgra
}
/// Convert L8 pixels (grayscale) to BGRA (B=G=R=lum, A=255).
fn convert_l8_to_bgra(pixels: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(pixels.len() * 4);
    for &lum in pixels {
        bgra.push(lum); // B
        bgra.push(lum); // G
        bgra.push(lum); // R
        bgra.push(255); // A
    }
    bgra
}
