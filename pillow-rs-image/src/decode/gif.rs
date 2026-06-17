//! GIF decoder — wraps the `gif` crate, returns raw palette indices (PIL parity).

use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;

/// Decode the first frame of a GIF into a [`DecodedImage`].
///
/// Uses `ColorOutput::Indexed` to return raw palette indices (1 byte/pixel).
/// This matches PIL's `image.tobytes()` for mode `P` (palette) images.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let mut decoder = gif::DecodeOptions::new();
    decoder.set_color_output(gif::ColorOutput::Indexed);
    let mut reader = decoder.read_info(Cursor::new(data)).ok()?;
    let frame = reader.read_next_frame().ok()??;

    let (w, h) = (frame.width as u32, frame.height as u32);
    let indices = frame.buffer.to_vec();

    Some(DecodedImage::new(w, h, indices, ColorType::L8))
}
