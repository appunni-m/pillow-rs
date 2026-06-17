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
                    pixels.push(val * 255 / 3);
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
                    pixels.push(val * 255 / 15);
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
