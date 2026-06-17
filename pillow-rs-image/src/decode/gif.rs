//! GIF decoder — wraps the `gif` crate, returns RGBA or RGB (alpha-stripped) pixels.

use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

/// Decode the first frame of a GIF into a [`DecodedImage`].
///
/// The decoder uses `ColorOutput::RGBA` so every pixel is 4 bytes.
/// If the frame has any non‑opaque pixel the output is `Rgba8`; otherwise
/// the alpha channel is stripped and the output is `Rgb8`.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = decoder.read_info(Cursor::new(data)).ok()?;
    let frame = reader.read_next_frame().ok()??;

    let (w, h) = (frame.width as u32, frame.height as u32);
    let buf = &frame.buffer;

    // Fast check: has transparency?
    let has_transparency = buf.chunks(4).any(|c| c[3] < 255);

    if has_transparency {
        Some(DecodedImage::new(w, h, buf.to_vec(), ColorType::Rgba8))
    } else {
        let rgb: Vec<u8> = buf.chunks(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
        Some(DecodedImage::new(w, h, rgb, ColorType::Rgb8))
    }
}
