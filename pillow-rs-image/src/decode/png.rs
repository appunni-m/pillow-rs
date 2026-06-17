//! PNG decoder using the `png` crate.
//!
//! Decodes all standard PNG color types (grayscale, grayscale+alpha, RGB, RGBA,
//! indexed/palette). Sub-8-bit indexed images are expanded to 1 byte/pixel
//! (matching PIL's mode "P" output). 16-bit channels are preserved as full
//! 16-bit data (2 bytes/channel, big-endian). Adam7 interlacing is handled
//! transparently by the underlying `png` crate.

use crate::types::{ColorType, DecodedImage};
use png::{BitDepth, ColorType as PngColorType};

/// Decode a PNG image from raw bytes.
///
/// Returns `Some(DecodedImage)` on success, or `None` if the data is not a
/// valid PNG or contains an unsupported configuration.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;

    let w = info.width;
    let h = info.height;

    match info.color_type {
        PngColorType::Grayscale => decode_grayscale(w, h, &buf, info.bit_depth),
        PngColorType::GrayscaleAlpha => match info.bit_depth {
            BitDepth::Eight => Some(DecodedImage::new(w, h, buf, ColorType::La8)),
            BitDepth::Sixteen => {
                // PIL's mode "LA" stores 16-bit data in little-endian byte order.
                // The png crate returns big-endian (per PNG spec), so swap byte order
                // for each 16-bit channel (luma, alpha).
                let mut pixels = buf.to_vec();
                for pair in pixels.chunks_exact_mut(2) {
                    pair.swap(0, 1);
                }
                Some(DecodedImage::new(w, h, pixels, ColorType::La16))
            }
            _ => None,
        },
        PngColorType::Rgb => match info.bit_depth {
            BitDepth::Eight => Some(DecodedImage::new(w, h, buf, ColorType::Rgb8)),
            BitDepth::Sixteen => {
                // PIL stores 16-bit channels in little-endian byte order.
                // The png crate returns big-endian (per PNG spec), so swap byte order
                // for each 16-bit channel (R, G, B).
                let mut pixels = buf.to_vec();
                for pair in pixels.chunks_exact_mut(2) {
                    pair.swap(0, 1);
                }
                Some(DecodedImage::new(w, h, pixels, ColorType::Rgb16))
            }
            _ => None,
        },
        PngColorType::Rgba => match info.bit_depth {
            BitDepth::Eight => Some(DecodedImage::new(w, h, buf, ColorType::Rgba8)),
            BitDepth::Sixteen => {
                // PIL stores 16-bit channels in little-endian byte order.
                // The png crate returns big-endian (per PNG spec), so swap byte order
                // for each 16-bit channel (R, G, B, A).
                let mut pixels = buf.to_vec();
                for pair in pixels.chunks_exact_mut(2) {
                    pair.swap(0, 1);
                }
                Some(DecodedImage::new(w, h, pixels, ColorType::Rgba16))
            }
            _ => None,
        },
        PngColorType::Indexed => {
            // Return palette indices (1 byte/pixel) — PIL mode "P" stores
            // palette indices as one byte per pixel. For sub-8-bit depths the
            // png crate returns packed data, so we must expand to 1 byte/px.
            let pixels = expand_indexed_palette(w, h, &buf, info.bit_depth);
            Some(DecodedImage::new(w, h, pixels, ColorType::L8))
        }
    }
}

/// Expand packed sub-8-bit palette indices to 1 byte per pixel.
///
/// PIL's mode "P" always stores palette indices as one byte per pixel, even
/// when the source PNG uses a smaller bit depth. The `png` crate returns
/// packed data for sub-8-bit depths, so we expand here.
fn expand_indexed_palette(w: u32, h: u32, buf: &[u8], bit_depth: BitDepth) -> Vec<u8> {
    let num_pixels = (w as u64 * h as u64) as usize;
    match bit_depth {
        BitDepth::One => {
            let row_bytes = w.div_ceil(8) as usize;
            let mut pixels = Vec::with_capacity(num_pixels);
            for row in 0..h as usize {
                let start = row * row_bytes;
                for col in 0..w as usize {
                    let byte = buf.get(start + col / 8).copied().unwrap_or(0);
                    let shift = 7 - (col % 8);
                    pixels.push((byte >> shift) & 1);
                }
            }
            pixels
        }
        BitDepth::Two => {
            let row_bytes = (w * 2).div_ceil(8) as usize;
            let mut pixels = Vec::with_capacity(num_pixels);
            for row in 0..h as usize {
                let start = row * row_bytes;
                for col in 0..w as usize {
                    let byte = buf.get(start + col / 4).copied().unwrap_or(0);
                    let shift = 6 - ((col % 4) * 2);
                    pixels.push((byte >> shift) & 3);
                }
            }
            pixels
        }
        BitDepth::Four => {
            let row_bytes = (w * 4).div_ceil(8) as usize;
            let mut pixels = Vec::with_capacity(num_pixels);
            for row in 0..h as usize {
                let start = row * row_bytes;
                for col in 0..w as usize {
                    let byte = buf.get(start + col / 2).copied().unwrap_or(0);
                    pixels.push(if col % 2 == 0 { byte >> 4 } else { byte & 0x0F });
                }
            }
            pixels
        }
        // 8-bit palette is already 1 byte/pixel
        _ => buf.to_vec(),
    }
}

/// Decode a grayscale PNG, handling sub-8-bit packed pixel formats.
fn decode_grayscale(w: u32, h: u32, buf: &[u8], bit_depth: BitDepth) -> Option<DecodedImage> {
    let num_pixels = (w as u64 * h as u64) as usize;
    match bit_depth {
        BitDepth::One => {
            // Return packed 1-bit data (MSB first, 8 pixels per byte) — PIL's
            // `tobytes()` for mode "1" returns packed bits, not expanded bytes.
            // The png crate already delivers packed scanlines in `buf`.
            Some(DecodedImage::new(w, h, buf.to_vec(), ColorType::L8))
        }
        BitDepth::Two => {
            // 2-bit: each byte holds 4 pixels (2 bits each, MSB first)
            let row_bytes = (w * 2).div_ceil(8) as usize;
            let mut pixels = Vec::with_capacity(num_pixels);
            for row in 0..h as usize {
                let start = row * row_bytes;
                for col in 0..w as usize {
                    let byte = buf.get(start + col / 4).copied().unwrap_or(0);
                    let shift = 6 - ((col % 4) * 2);
                    let val = (byte >> shift) & 3;
                    // Scale 0..3 to 0..255
                    pixels.push((val * 255 / 3));
                }
            }
            Some(DecodedImage::new(w, h, pixels, ColorType::L8))
        }
        BitDepth::Four => {
            // 4-bit: each byte holds 2 pixels (high nibble first, MSB-first)
            let row_bytes = (w * 4).div_ceil(8) as usize;
            let mut pixels = Vec::with_capacity(num_pixels);
            for row in 0..h as usize {
                let start = row * row_bytes;
                for col in 0..w as usize {
                    let byte = buf.get(start + col / 2).copied().unwrap_or(0);
                    let val = if col % 2 == 0 { byte >> 4 } else { byte & 0x0F };
                    // Scale 0..15 to 0..255
                    pixels.push((val * 255 / 15));
                }
            }
            Some(DecodedImage::new(w, h, pixels, ColorType::L8))
        }
        BitDepth::Eight => Some(DecodedImage::new(w, h, buf.to_vec(), ColorType::L8)),
        BitDepth::Sixteen => {
            // PIL's mode "I;16" stores 16-bit data in little-endian byte order.
            // The png crate returns big-endian (per PNG spec), so swap byte order.
            let mut pixels = buf.to_vec();
            for pair in pixels.chunks_exact_mut(2) {
                pair.swap(0, 1);
            }
            Some(DecodedImage::new(w, h, pixels, ColorType::L16))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal 1x1 white 8-bit grayscale PNG.
    fn minimal_gray_png() -> Vec<u8> {
        // Manually constructed minimal PNG: 1x1 grayscale, value 255
        let mut png = Vec::new();
        // Signature
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        // IHDR chunk: 1x1, 8-bit grayscale
        let ihdr_data = [
            0, 0, 0, 1, // width = 1
            0, 0, 0, 1, // height = 1
            8, // bit depth = 8
            0, // color type = grayscale
            0, 0, 0, 0, // compression, filter, interlace
        ];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&ihdr_data);
        let crc = crc32(&ihdr);
        png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
        png.extend_from_slice(&ihdr);
        png.extend_from_slice(&crc.to_be_bytes());

        // IDAT chunk: uncompressed raw scanline (filter byte 0 + pixel 255)
        // Raw data: filter=0 (None), pixel=255
        // Deflate: stored block (no compression) for 2 bytes
        let raw = [0u8, 255];
        let deflated = deflate_raw(&raw);
        let mut idat = Vec::new();
        idat.extend_from_slice(b"IDAT");
        idat.extend_from_slice(&deflated);
        let crc = crc32(&idat);
        png.extend_from_slice(&(deflated.len() as u32).to_be_bytes());
        png.extend_from_slice(&idat);
        png.extend_from_slice(&crc.to_be_bytes());

        // IEND chunk
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

    /// Minimal deflate stored block (no compression, no zlib wrapper).
    fn deflate_raw(data: &[u8]) -> Vec<u8> {
        // zlib wrapper: 2 bytes (CMF + FLG)
        let cmf = 0x78; // deflate, window size 32K
        let flg = 0x01; // check bits
        let mut out = vec![cmf, flg];
        // Deflate stored block
        let len = data.len() as u16;
        let nlen = !len;
        out.push(1); // BFINAL=1, BTYPE=00 (stored)
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(data);
        // Adlers-32 checksum
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

    #[test]
    fn test_decode_grayscale_8bit() {
        let png = minimal_gray_png();
        let img = decode(&png).expect("should decode");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.color, ColorType::L8);
        assert_eq!(img.pixels, vec![255]);
    }
}
