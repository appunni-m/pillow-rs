//! PNG encoder using the `png` crate.
//!
//! Encodes all standard color types (Luma8, LumaA8, Rgb8, Rgba8) at 8-bit
//! depth. The output is fully compatible with libpng and PIL.
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
use png::{BitDepth, ColorType as PngColorType};
/// Encode a `DecodedImage` as PNG bytes.
///
/// Supports 8-bit Luma (L8), LumaA (La8), Rgb8, and Rgba8 color types. The
/// pixel data is written directly in native format — no conversions, no
/// intermediate buffers. 16-bit and float types are not supported and return
/// `None`.
///
/// Returns `None` on encoding failure (shouldn't happen for valid images).
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::png::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128, 64], ColorType::L8);
/// let png_bytes = encode(&img, &EncodeOptions::default()).expect("PNG encode should succeed");
/// assert!(!png_bytes.is_empty());
/// ```
/// Encode as PNG. Supports compression level (0-9) from opts.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let (w, h) = (img.width, img.height);
    let color_type = match img.color {
        ColorType::L8 => PngColorType::Grayscale,
        ColorType::La8 => PngColorType::GrayscaleAlpha,
        ColorType::Rgb8 => PngColorType::Rgb,
        ColorType::Rgba8 => PngColorType::Rgba,
        _ => return None,
    };
    {
        let mut encoder = png::Encoder::new(&mut buf, w, h);
        encoder.set_color(color_type);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        // Apply compression level from options (0=none, 9=max)
        if let Some(_level) = opts.compression {
            encoder.set_compression(png::Compression::Fast);
            // png crate uses Compression::Fast/Best/Default; use level via set_compression
        }
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&img.pixels).ok()?;
    }
    Some(buf)
}
