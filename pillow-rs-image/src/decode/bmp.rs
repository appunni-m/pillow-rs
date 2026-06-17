//! BMP decoder — pure Rust, no external crate.
//!
//! Handles:
//! - BITMAPFILEHEADER + BITMAPINFOHEADER (and larger DIB headers)
//! - Bit depths: 1, 4, 8, 16 (RGB555 / BI_BITFIELDS), 24, 32 (RGBA)
//! - Compression: BI_RGB (none), BI_RLE8, BI_RLE4, BI_BITFIELDS
//! - Bottom‑up (positive height) and top‑down (negative height)
//! - 4‑byte row padding
//! - Palette (color table)

use crate::types::{ColorType, DecodedImage};
use std::io::{Cursor, Read, Seek, SeekFrom};

// ---------------------------------------------------------------------------
// Little‑endian helpers
// ---------------------------------------------------------------------------

fn read_u16_le<R: Read>(r: &mut R) -> Option<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).ok()?;
    Some(u16::from_le_bytes(b))
}

fn read_u32_le<R: Read>(r: &mut R) -> Option<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).ok()?;
    Some(u32::from_le_bytes(b))
}

fn read_i32_le<R: Read>(r: &mut R) -> Option<i32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).ok()?;
    Some(i32::from_le_bytes(b))
}

/// Row size in bytes (padded to 4‑byte boundary).
fn row_size(bits_per_pixel: u16, width: u32) -> usize {
    (((bits_per_pixel as u64) * (width as u64) + 31) / 32 * 4) as usize
}

// ---------------------------------------------------------------------------
// Palette reading
// ---------------------------------------------------------------------------

/// Read the color palette. Each entry is 4 bytes (B, G, R, reserved).
/// Returns a flat `[B, G, R, B, G, R, …]` slice (4 bytes per palette entry,
/// but we keep the full quad for stride simplicity).
fn read_palette(r: &mut Cursor<&[u8]>, count: u32) -> Option<Vec<[u8; 4]>> {
    let mut pal = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut entry = [0u8; 4];
        r.read_exact(&mut entry).ok()?; // B, G, R, reserved
        pal.push(entry);
    }
    Some(pal)
}

// ---------------------------------------------------------------------------
// Channel extraction from bit‑mask (for BI_BITFIELDS)
// ---------------------------------------------------------------------------

fn extract_channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let width = (mask >> shift).count_ones();
    let value = (pixel & mask) >> shift;
    if width >= 8 {
        (value >> (width - 8)) as u8
    } else {
        let max_val = (1u32 << width) - 1;
        ((value * 255) / max_val) as u8
    }
}

// ---------------------------------------------------------------------------
// RLE8 decoder
// ---------------------------------------------------------------------------

fn decode_rle8(data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; width * height];
    let mut x = 0usize;
    let mut y = 0usize;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let count = data[i] as usize;
        let value = data[i + 1];
        i += 2;

        if count > 0 {
            // Encoded mode: repeat `value` `count` times
            for _ in 0..count {
                if x < width {
                    out[y * width + x] = value;
                }
                x += 1;
            }
        } else {
            // Escape sequences
            match value {
                0 => {
                    // End of line
                    x = 0;
                    y += 1;
                    if y >= height {
                        break;
                    }
                }
                1 => break, // End of bitmap
                2 => {
                    // Delta
                    if i + 1 >= data.len() {
                        return None;
                    }
                    let dx = data[i] as usize;
                    let dy = data[i + 1] as usize;
                    i += 2;
                    x += dx;
                    y += dy;
                    if y >= height {
                        break;
                    }
                }
                _ => {
                    // Absolute mode: `value` literal bytes follow, padded to word boundary
                    let abs_len = value as usize;
                    if i + abs_len > data.len() {
                        return None;
                    }
                    for j in 0..abs_len {
                        if x < width {
                            out[y * width + x] = data[i + j];
                        }
                        x += 1;
                    }
                    i += abs_len;
                    // Pad to 16-bit boundary
                    if abs_len % 2 == 1 {
                        i += 1;
                    }
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// RLE4 decoder
// ---------------------------------------------------------------------------

fn decode_rle4(data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; width * height];
    let mut x = 0usize;
    let mut y = 0usize;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let count = data[i] as usize;
        let value = data[i + 1];
        i += 2;

        if count > 0 {
            // Encoded mode: the two nibbles of `value` are repeated `count` times
            let hi = (value >> 4) & 0x0F;
            let lo = value & 0x0F;
            for _ in 0..count {
                if x < width {
                    out[y * width + x] = hi;
                }
                x += 1;
                if x < width {
                    out[y * width + x] = lo;
                }
                x += 1;
            }
        } else {
            // Escape sequences
            match value {
                0 => {
                    // End of line
                    x = 0;
                    y += 1;
                    if y >= height {
                        break;
                    }
                }
                1 => break, // End of bitmap
                2 => {
                    // Delta
                    if i + 1 >= data.len() {
                        return None;
                    }
                    let dx = data[i] as usize;
                    let dy = data[i + 1] as usize;
                    i += 2;
                    x += dx;
                    y += dy;
                    if y >= height {
                        break;
                    }
                }
                _ => {
                    // Absolute mode: `value` nibbles follow
                    let nibble_count = value as usize;
                    let byte_count = (nibble_count + 1) / 2;
                    if i + byte_count > data.len() {
                        return None;
                    }
                    for j in 0..nibble_count {
                        let byte = data[i + j / 2];
                        let nibble = if j % 2 == 0 {
                            (byte >> 4) & 0x0F
                        } else {
                            byte & 0x0F
                        };
                        if x < width {
                            out[y * width + x] = nibble;
                        }
                        x += 1;
                    }
                    i += byte_count;
                    // Pad to 16-bit boundary
                    if byte_count % 2 == 1 {
                        i += 1;
                    }
                }
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Main decoder
// ---------------------------------------------------------------------------

pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut r = Cursor::new(data);

    // --- BITMAPFILEHEADER (14 bytes) ---
    let mut magic = [0u8; 2];
    r.read_exact(&mut magic).ok()?;
    if &magic != b"BM" {
        return None;
    }
    let _file_size = read_u32_le(&mut r)?; // bytes 2-5
    r.seek(SeekFrom::Current(4)).ok()?; // bytes 6-9 (reserved)
    let data_offset = read_u32_le(&mut r)? as u64; // bytes 10-13

    // --- DIB header ---
    let header_size = read_u32_le(&mut r)?;
    let width = read_i32_le(&mut r)?;
    let height = read_i32_le(&mut r)?;
    let _planes = read_u16_le(&mut r)?;
    let bit_depth = read_u16_le(&mut r)?;
    let compression = read_u32_le(&mut r)?;
    let _image_size = read_u32_le(&mut r)?;
    let _x_pels = read_i32_le(&mut r)?;
    let _y_pels = read_i32_le(&mut r)?;
    let colors_used = read_u32_le(&mut r)?;
    let _colors_important = read_u32_le(&mut r)?;

    // After the standard 40-byte fields, the cursor is at position 14 + 40 = 54.
    let pos_after_standard = r.position();

    // Top-down if height is negative.
    let top_down = height < 0;
    let h = height.unsigned_abs();
    let w = width as u32;
    if w == 0 || h == 0 {
        return None;
    }

    // --- BI_BITFIELDS masks ---
    let (rm, gm, bm, am): (u32, u32, u32, u32) = if compression == 3 {
        match header_size {
            40 => {
                // Masks follow immediately after the 40-byte header.
                r.seek(SeekFrom::Start(pos_after_standard)).ok()?;
                let r0 = read_u32_le(&mut r)?;
                let g0 = read_u32_le(&mut r)?;
                let b0 = read_u32_le(&mut r)?;
                let a0 = if bit_depth == 32 {
                    read_u32_le(&mut r).unwrap_or(0)
                } else {
                    0
                };
                (r0, g0, b0, a0)
            }
            _ => {
                // For V4/V5 headers the masks are embedded at known offsets.
                // V4 offsets (from DIB start): red=44, green=48, blue=52, alpha=56
                r.seek(SeekFrom::Start(14 + 44)).ok()?;
                let r0 = read_u32_le(&mut r)?;
                let g0 = read_u32_le(&mut r)?;
                let b0 = read_u32_le(&mut r)?;
                let a0 = if bit_depth == 32 && header_size >= 60 {
                    read_u32_le(&mut r).unwrap_or(0)
                } else {
                    0
                };
                (r0, g0, b0, a0)
            }
        }
    } else {
        // Default RGB555 masks for 16-bit BI_RGB.
        if bit_depth == 16 {
            (0x7C00, 0x03E0, 0x001F, 0)
        } else {
            (0, 0, 0, 0)
        }
    };

    // --- Skip any remaining DIB header bytes to reach palette area ---
    let dib_end = 14u64 + header_size as u64;
    if r.position() < dib_end {
        r.seek(SeekFrom::Start(dib_end)).ok()?;
    }

    // --- Palette (color table) ---
    let pal_count = if colors_used > 0 {
        colors_used
    } else if bit_depth <= 8 {
        1u32 << bit_depth
    } else {
        0
    };

    let _palette = if pal_count > 0 {
        read_palette(&mut r, pal_count)?
    } else {
        Vec::new()
    };

    // --- Seek to pixel data ---
    if r.position() != data_offset {
        r.seek(SeekFrom::Start(data_offset)).ok()?;
    }

    // ------------------------------------------------------------------
    // Pixel decoding
    // ------------------------------------------------------------------
    let width_usize = w as usize;
    let height_usize = h as usize;

    let pixels: Vec<u8> = if compression == 1 {
        // BI_RLE8 — return raw palette indices
        let mut remaining = Vec::new();
        r.read_to_end(&mut remaining).ok()?;
        decode_rle8(&remaining, width_usize, height_usize)?
    } else if compression == 2 {
        // BI_RLE4 — return raw palette indices
        let mut remaining = Vec::new();
        r.read_to_end(&mut remaining).ok()?;
        decode_rle4(&remaining, width_usize, height_usize)?
    } else {
        // BI_RGB or BI_BITFIELDS — uncompressed scanlines
        let stride = row_size(bit_depth, w);
        let mut raw = vec![0u8; stride * height_usize];
        r.read_exact(&mut raw).ok()?;

        match bit_depth {
            1 => {
                // 1 bpp — packed bits, skip stride padding (PIL mode '1' parity)
                let packed_per_row = (width_usize + 7) / 8;
                let mut out = Vec::with_capacity(packed_per_row * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    out.extend_from_slice(&raw[offset..offset + packed_per_row]);
                }
                out
            }
            4 => {
                // 4 bpp — expand nibbles to full-byte indices
                let mut out = Vec::with_capacity(width_usize * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let byte = raw[offset + col / 2];
                        let idx = if col % 2 == 0 { (byte >> 4) & 0x0F } else { byte & 0x0F };
                        out.push(idx);
                    }
                }
                out
            }
            8 => {
                // 8 bpp — raw palette indices, skip stride padding (PIL mode 'P' parity)
                let mut out = Vec::with_capacity(width_usize * height_usize);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    out.extend_from_slice(&raw[offset..offset + width_usize]);
                }
                out
            }
            16 => {
                // 16 bpp — RGB555 or BI_BITFIELDS
                let mut out = Vec::with_capacity(width_usize * height_usize * 3);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let lo = raw[offset + col * 2] as u32;
                        let hi = raw[offset + col * 2 + 1] as u32;
                        let pixel = lo | (hi << 8);
                        let rv = extract_channel(pixel, rm);
                        let gv = extract_channel(pixel, gm);
                        let bv = extract_channel(pixel, bm);
                        out.extend_from_slice(&[rv, gv, bv]);
                    }
                }
                out
            }
            24 => {
                // 24 bpp — BGR order
                let mut out = Vec::with_capacity(width_usize * height_usize * 3);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let b = raw[offset + col * 3];
                        let g = raw[offset + col * 3 + 1];
                        let r = raw[offset + col * 3 + 2];
                        out.extend_from_slice(&[r, g, b]);
                    }
                }
                out
            }
            32 => {
                // 32 bpp — BGRA → RGB (strip alpha, PIL parity)
                let mut out = Vec::with_capacity(width_usize * height_usize * 3);
                for row in 0..height_usize {
                    let src_row = if top_down { row } else { height_usize - 1 - row };
                    let offset = src_row * stride;
                    for col in 0..width_usize {
                        let b = raw[offset + col * 4];
                        let g = raw[offset + col * 4 + 1];
                        let r = raw[offset + col * 4 + 2];
                        // Skip alpha byte
                        out.extend_from_slice(&[r, g, b]);
                    }
                }
                out
            }
            _ => return None,
        }
    };

    // Determine output color type
    let color = if bit_depth <= 8 || compression == 1 || compression == 2 {
        ColorType::L8
    } else if bit_depth == 32 {
        ColorType::Rgb8
    } else if bit_depth == 16 && am != 0 {
        ColorType::Rgba8
    } else {
        ColorType::Rgb8
    };

    Some(DecodedImage::new(w, h, pixels, color))
}
