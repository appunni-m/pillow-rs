//! TIFF encoder — wraps the `tiff` crate.
//!
//! Supports L8 (Grayscale 8-bit), Rgb8 (RGB 24-bit), and Rgba8 (RGBA 32-bit)
//! color types. Uses uncompressed encoding for maximum compatibility.
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use std::io::Cursor;
use tiff::encoder::{colortype, TiffEncoder};
/// Encode a `DecodedImage` as TIFF bytes.
///
/// Maps color types to TIFF photometric interpretations:
/// - `L8` → `Gray8` (BlackIsZero)
/// - `Rgb8` → `RGB8`
/// - `Rgba8` → `RGBA8`
///
/// Returns `None` for unsupported color types or zero-dimension images.
pub fn encode(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let w = img.width;
    let h = img.height;
    if w == 0 || h == 0 {
        return None;
    }
    let mut buf = Vec::new();
    // Wrap in Cursor for Write+Seek required by TiffEncoder
    let mut cursor = Cursor::new(&mut buf);
    match img.color {
        ColorType::L8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::Gray8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        ColorType::Rgb8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::RGB8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        ColorType::Rgba8 => {
            let mut tiff = TiffEncoder::new(&mut cursor).ok()?;
            tiff.write_image::<colortype::RGBA8>(w, h, &img.pixels)
                .ok()?;
            Some(buf)
        }
        _ => None,
    }
}
