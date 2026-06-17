//! ICO decoder — parses ICO container format and delegates to PNG or BMP decoders.
//!
//! ICO (Icon) files store one or more icon images in a container that references
//! either embedded PNG data or BMP/DIB data for each entry. This decoder:
//!
//! 1. Parses the ICO header to get the entry count.
//! 2. Reads the directory entries (each 16 bytes).
//! 3. Selects the entry with the largest resolution (preferring 256x256).
//! 4. Dispatches to the PNG decoder if the entry data starts with the PNG
//!    signature, or attempts BMP/DIB decoding otherwise.
//!
//! Reference: https://en.wikipedia.org/wiki/ICO_(file_format)

use crate::decode::png;
use crate::types::{ColorType, DecodedImage};

/// ICO header size: 6 bytes
const ICO_HEADER_SIZE: usize = 6;

/// Directory entry size: 16 bytes
const ICO_DIR_ENTRY_SIZE: usize = 16;

/// Decode an ICO image from raw bytes.
///
/// Returns `Some(DecodedImage)` for the best icon entry found, or `None` if
/// the data is not valid ICO or no entry could be decoded.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    // ICO header: reserved(2) + type(2) + count(2)
    if data.len() < ICO_HEADER_SIZE {
        return None;
    }

    let reserved = u16::from_le_bytes([data[0], data[1]]);
    let icon_type = u16::from_le_bytes([data[2], data[3]]);
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;

    // Reserved should be 0; type 1 = ICO, type 2 = CUR
    if reserved != 0 {
        return None;
    }
    if icon_type != 1 && icon_type != 2 {
        return None;
    }
    if count == 0 || count > 255 {
        return None;
    }

    // Read all directory entries
    let entries_start = ICO_HEADER_SIZE;
    let entries_end = entries_start + count * ICO_DIR_ENTRY_SIZE;
    if data.len() < entries_end {
        return None;
    }

    // Find the best entry: prefer 256x256, then largest image
    let mut best_idx = 0;
    let mut best_score: u32 = 0;

    for i in 0..count {
        let entry_offset = entries_start + i * ICO_DIR_ENTRY_SIZE;
        let entry = &data[entry_offset..entry_offset + ICO_DIR_ENTRY_SIZE];

        let w = entry[0] as u32;
        let h = entry[1] as u32;
        // Width/height of 0 means 256 pixels
        let actual_w = if w == 0 { 256 } else { w };
        let actual_h = if h == 0 { 256 } else { h };

        let score = actual_w.saturating_mul(actual_h);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    // Decode the best entry
    decode_entry(data, best_idx, count)
}

/// Decode a single ICO directory entry by index.
fn decode_entry(data: &[u8], index: usize, _count: usize) -> Option<DecodedImage> {
    let entry_offset = ICO_HEADER_SIZE + index * ICO_DIR_ENTRY_SIZE;
    let entry = data.get(entry_offset..entry_offset + ICO_DIR_ENTRY_SIZE)?;

    // Directory entry fields:
    //   byte 0:    width (0 = 256)
    //   byte 1:    height (0 = 256)
    //   byte 2:    palette colors (0 if >= 256)
    //   byte 3:    reserved (0)
    //   bytes 4-5: color planes (should be 0 or 1)
    //   bytes 6-7: bits per pixel
    //   bytes 8-11: size of entry data in bytes
    //   bytes 12-15: offset of entry data from start of file
    let _w = entry[0];
    let _h = entry[1];
    let _palette = entry[2];
    let _reserved = entry[3];
    let _planes = u16::from_le_bytes([entry[4], entry[5]]);
    let _bpp = u16::from_le_bytes([entry[6], entry[7]]);
    let data_size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
    let data_offset =
        u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;

    // Validate bounds
    if data_size == 0 || data_offset == 0 {
        return None;
    }
    let entry_data_start = data_offset;
    let entry_data_end = entry_data_start + data_size;

    let entry_data = data.get(entry_data_start..entry_data_end)?;

    // Check if the entry data is PNG (magic: 0x89 0x50 0x4E 0x47)
    if entry_data.len() >= 8 && entry_data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        // Decode as PNG
        png::decode(entry_data)
    } else {
        // BMP/DIB data inside ICO
        // ICO BMP data starts with a BITMAPINFOHEADER (40 bytes) at offset 0,
        // but without the standard BMP file header (no "BM" signature).
        // We extract the pixel data manually.
        decode_ico_bmp(entry_data, entry)
    }
}

/// Decode an embedded BMP/DIB entry inside an ICO file.
///
/// ICO-embedded BMP data differs from standalone BMPs:
///   - No "BM" file header (starts directly with BITMAPINFOHEADER)
///   - Pixel data is uncompressed and stored in a specific layout
fn decode_ico_bmp(data: &[u8], _entry: &[u8]) -> Option<DecodedImage> {
    if data.len() < 40 {
        return None;
    }

    // BITMAPINFOHEADER fields
    let _header_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let width = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let height = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    // ICO height is doubled in BMP header (AND mask row is included)
    let actual_height = height / 2;

    let _planes = u16::from_le_bytes([data[12], data[13]]);
    let bpp = u16::from_le_bytes([data[14], data[15]]);
    let _compression = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let _image_size = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let _colors_used = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

    if width == 0 || actual_height == 0 || width > 16384 || actual_height > 16384 {
        return None;
    }

    match bpp {
        32 => decode_ico_bmp_32bpp(data, width, actual_height),
        24 => decode_ico_bmp_24bpp(data, width, actual_height),
        8 => decode_ico_bmp_8bpp(data, width, actual_height),
        4 => decode_ico_bmp_4bpp(data, width, actual_height),
        1 => decode_ico_bmp_1bpp(data, width, actual_height),
        _ => None,
    }
}

/// Decode a 32-bit BGRA ICO BMP entry (4 bytes/pixel).
fn decode_ico_bmp_32bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    let row_size = width as usize * 4;
    // Each row is padded to a multiple of 4 bytes
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = header_size;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    let mut pixels = Vec::with_capacity(row_size * height as usize);

    // ICO BMP stores rows bottom-up; we flip to top-down
    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        // BGRA → RGBA conversion
        for chunk in row.chunks(4) {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            let a = chunk[3];
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 24-bit BGR ICO BMP entry (3 bytes/pixel).
fn decode_ico_bmp_24bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    let row_size = width as usize * 3;
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = header_size;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    let mut pixels = Vec::with_capacity(row_size * height as usize);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        for chunk in row.chunks(3) {
            let b = chunk[0];
            let g = chunk[1];
            let r = chunk[2];
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgb8))
}

/// Decode an 8-bit indexed ICO BMP entry (palette + indices).
fn decode_ico_bmp_8bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    // Palette: 256 colors * 4 bytes each (BGRA)
    let palette_size = 256 * 4;
    let palette_end = header_size + palette_size;

    let row_size = width as usize;
    let padded_row = (row_size + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette (BGRA → RGBA)
    let palette_raw = data.get(header_size..palette_end)?;
    let mut palette = Vec::with_capacity(256);
    for i in 0..256 {
        let offset = i * 4;
        if offset + 4 <= palette_raw.len() {
            let b = palette_raw[offset];
            let g = palette_raw[offset + 1];
            let r = palette_raw[offset + 2];
            let a = palette_raw[offset + 3];
            palette.push([r, g, b, a]);
        } else {
            palette.push([0, 0, 0, 255]);
        }
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_size;
        let row = &pixels_raw[row_start..row_end];

        for &idx in row {
            let color = palette[idx as usize];
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(color[3]);
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 4-bit indexed ICO BMP entry.
fn decode_ico_bmp_4bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    // Palette: 16 colors * 4 bytes each
    let palette_size = 16 * 4;
    let palette_end = header_size + palette_size;

    // 4bpp: 2 pixels per byte
    let row_bytes = (width as usize + 1) / 2;
    let padded_row = (row_bytes + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette
    let palette_raw = data.get(header_size..palette_end)?;
    let mut palette = Vec::with_capacity(16);
    for i in 0..16 {
        let offset = i * 4;
        if offset + 4 <= palette_raw.len() {
            let b = palette_raw[offset];
            let g = palette_raw[offset + 1];
            let r = palette_raw[offset + 2];
            let a = palette_raw[offset + 3];
            palette.push([r, g, b, a]);
        } else {
            palette.push([0, 0, 0, 255]);
        }
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_bytes;
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            let hi = (byte >> 4) & 0x0F;
            let lo = byte & 0x0F;
            if col < width as usize {
                let color = palette[hi as usize];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(color[3]);
            }
            col += 1;
            if col < width as usize {
                let color = palette[lo as usize];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(color[3]);
            }
            col += 1;
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

/// Decode a 1-bit indexed ICO BMP entry.
fn decode_ico_bmp_1bpp(data: &[u8], width: u32, height: u32) -> Option<DecodedImage> {
    let header_size = 40;
    // Palette: 2 colors * 4 bytes each
    let palette_size = 2 * 4;
    let palette_end = header_size + palette_size;

    // 1bpp: 8 pixels per byte
    let row_bytes = (width as usize + 7) / 8;
    let padded_row = (row_bytes + 3) & !3;
    let pixel_data_size = padded_row * height as usize;

    let pixel_start = palette_end;
    let pixel_end = pixel_start + pixel_data_size;
    let pixels_raw = data.get(pixel_start..pixel_end)?;

    // Read palette
    let palette_raw = data.get(header_size..palette_end)?;
    let mut palette = Vec::with_capacity(2);
    for i in 0..2 {
        let offset = i * 4;
        if offset + 4 <= palette_raw.len() {
            let b = palette_raw[offset];
            let g = palette_raw[offset + 1];
            let r = palette_raw[offset + 2];
            let a = palette_raw[offset + 3];
            palette.push([r, g, b, a]);
        } else {
            palette.push([0, 0, 0, 255]);
        }
    }

    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in (0..height as usize).rev() {
        let row_start = y * padded_row;
        let row_end = row_start + row_bytes;
        let row = &pixels_raw[row_start..row_end];

        let mut col = 0;
        for &byte in row {
            for bit in (0..8).rev() {
                if col >= width as usize {
                    break;
                }
                let idx = ((byte >> bit) & 1) as usize;
                let color = palette[idx];
                pixels.push(color[0]);
                pixels.push(color[1]);
                pixels.push(color[2]);
                pixels.push(color[3]);
                col += 1;
            }
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal ICO file containing a 32-bit BGRA BMP entry.
    ///
    /// The entry is 2x2 pixels: red, green, blue, white.
    fn build_ico_32bpp() -> Vec<u8> {
        let mut ico = Vec::new();

        // ICO header
        ico.extend_from_slice(&[0u8; 2]); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
        ico.extend_from_slice(&1u16.to_le_bytes()); // count = 1

        // Directory entry
        ico.push(2); // width = 2
        ico.push(2); // height = 2
        ico.push(0); // colors
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
        // data size and offset will be patched

        let data_offset = ico.len() + 8; // after size(4) + offset(4)
        let mut bmp_data = Vec::new();

        // BITMAPINFOHEADER (40 bytes)
        bmp_data.extend_from_slice(&40u32.to_le_bytes()); // header size
        bmp_data.extend_from_slice(&2u32.to_le_bytes()); // width
        bmp_data.extend_from_slice(&4u32.to_le_bytes()); // height (doubled for ICO: 2 + AND mask = 4)
        bmp_data.extend_from_slice(&1u16.to_le_bytes()); // planes
        bmp_data.extend_from_slice(&32u16.to_le_bytes()); // bpp
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // compression
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // image size
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // x pixels per meter
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // y pixels per meter
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // colors used
        bmp_data.extend_from_slice(&0u32.to_le_bytes()); // important colors

        // Pixel data (32-bit BGRA, bottom-up, each row padded to 4-byte boundary)
        // Row 1 (bottom): blue, white
        // Row 0 (top): red, green
        // Row 1: B G R A
        let row1 = vec![255u8, 0, 0, 255, 255, 255, 255, 255]; // blue, white (BGRA)
        let row0 = vec![0u8, 0, 255, 255, 0, 255, 0, 255]; // red, green (BGRA)
        bmp_data.extend_from_slice(&row1); // bottom row first
        bmp_data.extend_from_slice(&row0); // top row last
        // AND mask row (dummy)
        bmp_data.extend_from_slice(&[0u8; 4]);

        let data_size = bmp_data.len();

        // Patch size and offset in directory entry
        let _size_offset = ico.len() + 4; // skip forward 2 bytes for planes + 2 for bpp
        ico.extend_from_slice(&(data_size as u32).to_le_bytes());
        ico.extend_from_slice(&(data_offset as u32).to_le_bytes());

        ico.extend_from_slice(&bmp_data);

        ico
    }

    #[test]
    fn test_not_ico() {
        assert!(decode(b"not an ico").is_none());
    }

    #[test]
    fn test_ico_too_small() {
        assert!(decode(b"\x00\x00").is_none());
    }

    #[test]
    fn test_ico_empty() {
        let mut ico = vec![0u8; 6];
        ico[2] = 1;
        ico[3] = 0; // type = ICO
        ico[4] = 0;
        ico[5] = 0; // count = 0
        assert!(decode(&ico).is_none());
    }

    #[test]
    fn test_ico_bad_reserved() {
        let mut ico = vec![0u8; 6];
        ico[0] = 1; // non-zero reserved
        ico[2] = 1;
        ico[4] = 1;
        assert!(decode(&ico).is_none());
    }

    #[test]
    fn test_ico_32bpp_bmp_entry() {
        let ico = build_ico_32bpp();
        let img = decode(&ico).expect("should decode ICO with 32bpp BMP entry");
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.color, ColorType::Rgba8);
        assert_eq!(img.pixels.len(), 2 * 2 * 4);

        // Verify pixel colors (after BGRA→RGBA conversion + bottom-up flip)
        // BMP rows are stored bottom-up in file.
        // File row 0 (first) = bottom of image = BLUE, WHITE (BGRA)
        // File row 1 (second) = top of image = RED, GREEN (BGRA)
        // Decoder iterates y in reverse: processes row 1 first, then row 0.
        //
        // Top row of output: RED, GREEN
        //   File row 1: BGRA(0,0,255,255) → RGBA(255,0,0,255) = RED (top-left)
        //   File row 1: BGRA(0,255,0,255) → RGBA(0,255,0,255) = GREEN (top-right)
        // Bottom row of output: BLUE, WHITE
        //   File row 0: BGRA(255,0,0,255) → RGBA(0,0,255,255) = BLUE (bottom-left)
        //   File row 0: BGRA(255,255,255,255) → RGBA(255,255,255,255) = WHITE (bottom-right)
        assert_eq!(&img.pixels[0..4], &[255, 0, 0, 255]); // RED (top-left)
        assert_eq!(&img.pixels[4..8], &[0, 255, 0, 255]); // GREEN (top-right)
        assert_eq!(&img.pixels[8..12], &[0, 0, 255, 255]); // BLUE (bottom-left)
        assert_eq!(&img.pixels[12..16], &[255, 255, 255, 255]); // WHITE (bottom-right)
    }

    #[test]
    fn test_ico_png_entry() {
        // Build ICO wrapping a minimal PNG
        let minimal_png = build_minimal_png();
        let mut ico = Vec::new();

        // ICO header
        ico.extend_from_slice(&[0u8; 2]); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
        ico.extend_from_slice(&1u16.to_le_bytes()); // count = 1

        // Directory entry: width=1, height=1
        ico.push(1);
        ico.push(1);
        ico.push(0);
        ico.push(0);
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bpp

        let data_offset = ico.len() + 8;
        ico.extend_from_slice(&(minimal_png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(data_offset as u32).to_le_bytes());

        ico.extend_from_slice(&minimal_png);

        let img = decode(&ico).expect("should decode ICO with PNG entry");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        // PNG decoder returns Rgba8 for indexed images
        assert_eq!(img.pixels.len(), 4);
    }

    /// Build a minimal 1x1 white indexed PNG.
    fn build_minimal_png() -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

        // IHDR: 1x1, 8-bit indexed
        let ihdr_data = [
            0, 0, 0, 1, // width
            0, 0, 0, 1, // height
            8,          // bit depth
            3,          // color type = indexed
            0, 0, 0, 0, // compression, filter, interlace
        ];
        let ihdr_len = ihdr_data.len() as u32;
        let mut ihdr_chunk = Vec::new();
        ihdr_chunk.extend_from_slice(b"IHDR");
        ihdr_chunk.extend_from_slice(&ihdr_data);
        let crc = crc32(&ihdr_chunk);
        png.extend_from_slice(&ihdr_len.to_be_bytes());
        png.extend_from_slice(&ihdr_chunk);
        png.extend_from_slice(&crc.to_be_bytes());

        // PLTE: white (255, 255, 255)
        let plte_data = [255u8, 255, 255];
        let plte_len = plte_data.len() as u32;
        let mut plte_chunk = Vec::new();
        plte_chunk.extend_from_slice(b"PLTE");
        plte_chunk.extend_from_slice(&plte_data);
        let crc = crc32(&plte_chunk);
        png.extend_from_slice(&plte_len.to_be_bytes());
        png.extend_from_slice(&plte_chunk);
        png.extend_from_slice(&crc.to_be_bytes());

        // IDAT: uncompressed scanline
        let raw = [0u8, 0]; // filter=None + index 0
        let deflated = deflate_raw(&raw);
        let idat_len = deflated.len() as u32;
        let mut idat_chunk = Vec::new();
        idat_chunk.extend_from_slice(b"IDAT");
        idat_chunk.extend_from_slice(&deflated);
        let crc = crc32(&idat_chunk);
        png.extend_from_slice(&idat_len.to_be_bytes());
        png.extend_from_slice(&idat_chunk);
        png.extend_from_slice(&crc.to_be_bytes());

        // IEND
        let iend_len = 0u32;
        let mut iend_chunk = Vec::new();
        iend_chunk.extend_from_slice(b"IEND");
        let crc = crc32(&iend_chunk);
        png.extend_from_slice(&iend_len.to_be_bytes());
        png.extend_from_slice(&iend_chunk);
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
