//! WebP decoder — pure Rust stub. PIL parity requires implementing the WebP
//! bitstream format in Rust. Until then, this returns None (unsupported).

use crate::types::DecodedImage;

pub fn decode(_data: &[u8]) -> Option<DecodedImage> {
    None
}
