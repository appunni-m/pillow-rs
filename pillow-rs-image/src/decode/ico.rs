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
    let data_offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;

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
        #[cfg(feature = "png")]
        {
            crate::decode::png::decode(entry_data)
        }
        #[cfg(not(feature = "png"))]
        {
            None
        }
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
    let row_bytes = (width as usize).div_ceil(2);
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
    let row_bytes = (width as usize).div_ceil(8);
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
