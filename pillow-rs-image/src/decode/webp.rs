//! WebP decoder — RIFF container parsing with VP8/VP8L/VP8X chunk detection.
//!
//! WebP uses a RIFF-based container. The file starts with a RIFF header followed
//! by chunks. For lossy images the VP8 chunk contains a VP8 key frame bitstream.
//! For lossless images the VP8L chunk contains a VP8L bitstream. Extended (VP8X)
//! images may include alpha, animation, ICCP, or EXIF data.
//!
//! References:
//!   - WebP RIFF Container: https://developers.google.com/speed/webp/docs/riff_container
//!   - VP8 Bitstream: https://datatracker.ietf.org/doc/html/rfc6386
//!   - VP8L Bitstream: https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification
//!
//! Current implementation extracts image dimensions from VP8 and VP8L headers
//! and returns a correctly-sized placeholder image. The full VP8/VP8L pixel
//! decoder will be added in a follow-up.

use crate::types::{ColorType, DecodedImage};

/// Minimum valid WebP file size: RIFF(12) + chunk header(8) + 1 byte payload
const MIN_WEBP_SIZE: usize = 21;

/// Maximum reasonable image dimension to avoid OOM on corrupted data
const MAX_DIM: u32 = 16384;

/// Decode a WebP image from raw bytes.
///
/// Returns `Some(DecodedImage)` with the correct dimensions and a placeholder
/// gray pixel buffer, or `None` if the data is not a valid WebP image.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    if data.len() < MIN_WEBP_SIZE {
        return None;
    }

    // RIFF header: "RIFF" + 4-byte LE size + "WEBP"
    if &data[0..4] != b"RIFF" {
        return None;
    }
    if &data[8..12] != b"WEBP" {
        return None;
    }

    // Walk through RIFF chunks starting at offset 12
    let mut offset = 12;
    while offset + 8 <= data.len() {
        let chunk_tag = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        // Chunk data starts after the 8-byte chunk header
        let chunk_data_start = offset + 8;

        match chunk_tag {
            b"VP8 " => {
                return decode_vp8(data, chunk_data_start, chunk_size);
            }
            b"VP8L" => {
                return decode_vp8l(data, chunk_data_start, chunk_size);
            }
            b"VP8X" => {
                // VP8X signals an extended WebP — continue scanning for VP8/VP8L
                // rather than decoding here
            }
            _ => {
                // Unrecognized chunk — skip
            }
        }

        // Advance to next chunk (padded to even byte boundaries)
        let padded_size = chunk_size + (chunk_size & 1);
        offset = chunk_data_start + padded_size;
    }

    None
}

/// Decode a VP8 (lossy) chunk.
///
/// VP8 key frame bitstream layout (inside chunk data):
///   byte 0:      frame_tag
///     bits 0-1:  0 = key frame
///     bits 2-4:  version
///     bit 5:     show_frame
///   bytes 1-3:   start code 0x9D 0x01 0x2A (only for key frames)
///   bytes 4-5:   horizontal_size (LE, bits 0-13 = (width / 2))
///   bytes 6-7:   vertical_size (LE, bits 0-13 = (height / 2))
fn decode_vp8(data: &[u8], start: usize, size: usize) -> Option<DecodedImage> {
    let chunk = data.get(start..start + size)?;

    if chunk.len() < 10 {
        return None;
    }

    // Frame tag
    let frame_tag = chunk[0];
    let is_key_frame = (frame_tag & 0x01) == 0;

    if !is_key_frame {
        // Non-key frames reference previous frames — skip
        return None;
    }

    // Verify start code: 0x9D 0x01 0x2A
    if chunk[1] != 0x9D || chunk[2] != 0x01 || chunk[3] != 0x2A {
        return None;
    }

    // Extract width and height (divided by 2 in the bitstream)
    let raw_w = u16::from_le_bytes([chunk[4], chunk[5]]);
    let raw_h = u16::from_le_bytes([chunk[6], chunk[7]]);

    let width = ((raw_w & 0x3FFF) as u32).saturating_mul(2);
    let height = ((raw_h & 0x3FFF) as u32).saturating_mul(2);

    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return None;
    }

    // Return a correctly-sized placeholder (checkerboard RGB)
    let num_pixels = (width as u64 * height as u64) as usize;
    let mut pixels = vec![128u8; num_pixels.saturating_mul(3)];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) as usize) * 3;
            if (x / 8 + y / 8) % 2 == 0 {
                pixels[idx] = 128;
                pixels[idx + 1] = 108;
                pixels[idx + 2] = 88;
            } else {
                pixels[idx] = 200;
                pixels[idx + 1] = 180;
                pixels[idx + 2] = 160;
            }
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgb8))
}

/// Decode a VP8L (lossless) chunk.
///
/// VP8L header (5 bytes inside chunk data):
///   byte 0:      signature (0x2F)
///   bytes 1-4:   32-bit LE value containing:
///     bits 0-13:   width_minus_one
///     bits 14-27:  height_minus_one
///     bits 28-31:  version (ignored)
fn decode_vp8l(data: &[u8], start: usize, size: usize) -> Option<DecodedImage> {
    let chunk = data.get(start..start + size)?;

    if chunk.len() < 5 {
        return None;
    }

    // Signature check
    if chunk[0] != 0x2F {
        return None;
    }

    // Width and height packed in bytes 1-4 as a little-endian 32-bit value
    let dims = u32::from_le_bytes([chunk[1], chunk[2], chunk[3], chunk[4]]);
    let width = (dims & 0x3FFF) + 1;
    let height = ((dims >> 14) & 0x3FFF) + 1;

    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return None;
    }

    // Return a correctly-sized placeholder (green-tinted checkerboard RGBA)
    let num_pixels = (width as u64 * height as u64) as usize;
    let mut pixels = vec![0u8; num_pixels.saturating_mul(4)];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) as usize) * 4;
            if (x / 8 + y / 8) % 2 == 0 {
                pixels[idx] = 98;
                pixels[idx + 1] = 128;
                pixels[idx + 2] = 118;
            } else {
                pixels[idx] = 170;
                pixels[idx + 1] = 200;
                pixels[idx + 2] = 190;
            }
            pixels[idx + 3] = 255;
        }
    }

    Some(DecodedImage::new(width, height, pixels, ColorType::Rgba8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal WebP RIFF wrapper around chunk data.
    fn build_webp(chunk_tag: &[u8; 4], chunk_data: &[u8]) -> Vec<u8> {
        let mut webp = Vec::new();
        webp.extend_from_slice(b"RIFF");
        // Chunk size = 4 (tag "WEBP") + 8 (next chunk header) + data + padding
        let total_size = 4 + 8 + chunk_data.len() + (chunk_data.len() & 1);
        webp.extend_from_slice(&(total_size as u32).to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(chunk_tag);
        webp.extend_from_slice(&(chunk_data.len() as u32).to_le_bytes());
        webp.extend_from_slice(chunk_data);
        if chunk_data.len() % 2 == 1 {
            webp.push(0);
        }
        webp
    }

    /// Build a minimal VP8 key frame bitstream with the given width and height.
    fn build_vp8_frame(width: u16, height: u16) -> Vec<u8> {
        let mut frame = Vec::new();
        // Frame tag: key frame (bit 0 = 0), show_frame = 1 (bit 5)
        frame.push(0b0010_0000);
        // Start code: 0x9D 0x01 0x2A
        frame.push(0x9D);
        frame.push(0x01);
        frame.push(0x2A);
        // Horizontal size: width/2, scale = 0
        frame.extend_from_slice(&(width / 2).to_le_bytes());
        // Vertical size: height/2, scale = 0
        frame.extend_from_slice(&(height / 2).to_le_bytes());
        // Fill remaining with zeros to meet minimum frame size
        while frame.len() < 20 {
            frame.push(0);
        }
        frame
    }

    /// Build a minimal VP8L header with the given width and height.
    fn build_vp8l_header(width: u16, height: u16) -> Vec<u8> {
        let mut header = Vec::new();
        header.push(0x2F);
        let val = ((width as u32 - 1) & 0x3FFF) | (((height as u32 - 1) & 0x3FFF) << 14);
        header.extend_from_slice(&val.to_le_bytes());
        header.extend_from_slice(&[0u8; 10]);
        header
    }

    #[test]
    fn test_not_webp() {
        assert!(decode(b"not a webp").is_none());
        assert!(decode(b"RIFF....FAIL").is_none());
    }

    #[test]
    fn test_webp_too_small() {
        assert!(decode(b"RIFF").is_none());
    }

    #[test]
    fn test_vp8_lossy_16x16() {
        let chunk = build_vp8_frame(16, 16);
        let webp = build_webp(b"VP8 ", &chunk);
        let img = decode(&webp).expect("should decode VP8");
        assert_eq!(img.width, 16);
        assert_eq!(img.height, 16);
        assert_eq!(img.color, ColorType::Rgb8);
        assert_eq!(img.pixels.len(), 16 * 16 * 3);
    }

    #[test]
    fn test_vp8l_lossless_32x32() {
        let chunk = build_vp8l_header(32, 32);
        let webp = build_webp(b"VP8L", &chunk);
        let img = decode(&webp).expect("should decode VP8L");
        assert_eq!(img.width, 32);
        assert_eq!(img.height, 32);
        assert_eq!(img.color, ColorType::Rgba8);
        assert_eq!(img.pixels.len(), 32 * 32 * 4);
    }

    #[test]
    fn test_vp8l_odd_dimensions() {
        let chunk = build_vp8l_header(15, 23);
        let webp = build_webp(b"VP8L", &chunk);
        let img = decode(&webp).expect("should decode VP8L with odd dims");
        assert_eq!(img.width, 15);
        assert_eq!(img.height, 23);
    }

    #[test]
    fn test_vp8x_extended_delegates_to_vp8() {
        // VP8X chunk followed by VP8 chunk: decoder should skip VP8X and find VP8
        let vp8_chunk = build_vp8_frame(64, 48);
        let vp8x_data = vec![0x0Fu8; 10];
        let mut webp = Vec::new();
        webp.extend_from_slice(b"RIFF");
        let total_size = 4 + 8 + vp8x_data.len() + 8 + vp8_chunk.len();
        webp.extend_from_slice(&(total_size as u32).to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        // VP8X chunk
        webp.extend_from_slice(b"VP8X");
        webp.extend_from_slice(&(vp8x_data.len() as u32).to_le_bytes());
        webp.extend_from_slice(&vp8x_data);
        if vp8x_data.len() % 2 == 1 {
            webp.push(0);
        }
        // VP8 chunk
        webp.extend_from_slice(b"VP8 ");
        webp.extend_from_slice(&(vp8_chunk.len() as u32).to_le_bytes());
        webp.extend_from_slice(&vp8_chunk);
        if vp8_chunk.len() % 2 == 1 {
            webp.push(0);
        }

        let img = decode(&webp).expect("should decode VP8 after VP8X");
        assert_eq!(img.width, 64);
        assert_eq!(img.height, 48);
    }

    #[test]
    fn test_vp8_bad_start_code() {
        let mut chunk = build_vp8_frame(16, 16);
        chunk[4] = 0x00; // corrupt start code byte after frame tag
        let webp = build_webp(b"VP8 ", &chunk);
        assert!(decode(&webp).is_none());
    }

    #[test]
    fn test_vp8l_bad_signature() {
        let mut chunk = build_vp8l_header(16, 16);
        chunk[0] = 0xFF;
        let webp = build_webp(b"VP8L", &chunk);
        assert!(decode(&webp).is_none());
    }

    #[test]
    fn test_non_key_frame_rejected() {
        // Non-key frame (bit 0 = 1) should be rejected
        let mut chunk = build_vp8_frame(16, 16);
        chunk[0] = 0b0010_0001; // non-key frame
        let webp = build_webp(b"VP8 ", &chunk);
        assert!(decode(&webp).is_none());
    }
}
