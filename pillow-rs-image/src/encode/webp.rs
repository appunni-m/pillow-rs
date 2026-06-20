//! WebP encoder — pure Rust stub. PIL parity requires implementing the WebP
//! bitstream format in Rust. Until then, this returns None (unsupported).

use crate::encode_options::EncodeOptions;
use crate::types::DecodedImage;

pub fn encode(_img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    None
}
