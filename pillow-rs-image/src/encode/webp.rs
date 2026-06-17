//! WebP encoder — uses libwebp C library for lossless encoding.
//!
//! Both PIL and this encoder use the same libwebp under the hood, so output
//! for lossless encoding is pixel-identical. This is critical for roundtrip
//! tests that require exact binary equality.
//!
//! Supported color types:
//! - `Rgb8`  → WebPEncodeLosslessRGB
//! - `Rgba8` → WebPEncodeLosslessRGBA
//! - `L8`    → Expanded to RGB, then WebPEncodeLosslessRGB
//! - `La8`   → Alpha stripped, expanded to RGB, then WebPEncodeLosslessRGB
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};
// ---------------------------------------------------------------------------
// libwebp C FFI declarations — encoder functions
// ---------------------------------------------------------------------------
extern "C" {
    /// Lossless WebP encode from RGB (3 bytes/pixel).
    /// Returns size of output buffer (0 on failure).
    /// Output buffer must be freed with WebPFree().
    fn WebPEncodeLosslessRGB(
        rgb: *const u8,
        width: i32,
        height: i32,
        stride: i32,
        output: *mut *mut u8,
    ) -> usize;
    /// Lossless WebP encode from RGBA (4 bytes/pixel).
    fn WebPEncodeLosslessRGBA(
        rgba: *const u8,
        width: i32,
        height: i32,
        stride: i32,
        output: *mut *mut u8,
    ) -> usize;
    /// Free a buffer allocated by libwebp.
    fn WebPFree(ptr: *mut u8);
}
// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------
/// Encode a `DecodedImage` as lossless WebP bytes using libwebp.
///
/// Supported color types:
/// - `Rgb8`  → 3 bytes/pixel, direct lossless RGB WebP
/// - `Rgba8` → 4 bytes/pixel, direct lossless RGBA WebP
/// - `L8`    → 1 byte/pixel, expanded to RGB before encoding
/// - `La8`   → 2 bytes/pixel, alpha stripped and expanded to RGB before encoding
///
/// Unsupported types (L16, La16, Rgb16, Rgba16, Rgb32F, Rgba32F) return `None`.
///
/// # Examples
///
/// ```
/// use pillow_rs_image::types::{DecodedImage, ColorType};
/// use pillow_rs_image::encode::webp::encode;
///
/// let img = DecodedImage::new(2, 2, vec![255, 0, 128, 0, 255, 64], ColorType::Rgb8);
/// let webp = encode(&img, &EncodeOptions::default()).expect("WebP encode should succeed");
/// assert!(!webp.is_empty());
/// ```
pub fn encode(img: &DecodedImage, _opts: &EncodeOptions) -> Option<Vec<u8>> {
    let (w, h) = (img.width, img.height);
    let w_i32 = w as i32;
    let h_i32 = h as i32;
    match img.color {
        ColorType::Rgb8 => {
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGB(img.pixels.as_ptr(), w_i32, h_i32, stride, &mut output)
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::Rgba8 => {
            let stride = w_i32 * 4;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size = unsafe {
                WebPEncodeLosslessRGBA(img.pixels.as_ptr(), w_i32, h_i32, stride, &mut output)
            };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::L8 => {
            // Expand L8 (1 byte) to RGB (3 bytes): copy luma to R, G, B
            let num_pixels = (w as usize).saturating_mul(h as usize);
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for &l in &img.pixels {
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size =
                unsafe { WebPEncodeLosslessRGB(rgb.as_ptr(), w_i32, h_i32, stride, &mut output) };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        ColorType::La8 => {
            // Strip alpha, then expand to RGB: 2 bytes → 1 byte luma → 3 bytes RGB
            let num_pixels = (w as usize).saturating_mul(h as usize);
            let mut rgb = Vec::with_capacity(num_pixels * 3);
            for c in img.pixels.chunks_exact(2) {
                let l = c[0];
                rgb.push(l);
                rgb.push(l);
                rgb.push(l);
            }
            let stride = w_i32 * 3;
            let mut output: *mut u8 = std::ptr::null_mut();
            let size =
                unsafe { WebPEncodeLosslessRGB(rgb.as_ptr(), w_i32, h_i32, stride, &mut output) };
            if size == 0 || output.is_null() {
                return None;
            }
            let result = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
            unsafe { WebPFree(output) };
            Some(result)
        }
        _ => None,
    }
}
