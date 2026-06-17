//! WebP decoder — uses libwebp C library for pixel-perfect PIL parity.
//!
//! Both PIL and this decoder use the same libwebp code under the hood, so
//! the output is guaranteed to be byte-identical. This is critical for the
//! decode fixture tests which require exact binary equality with PIL.

use crate::types::{ColorType, DecodedImage};

// ---------------------------------------------------------------------------
// libwebp C FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    /// Get WebP image width and height (returns 0 on failure).
    fn WebPGetInfo(
        data: *const u8,
        data_size: usize,
        width: *mut i32,
        height: *mut i32,
    ) -> i32;

    /// Decode WebP image to RGB (3 bytes/pixel). Returns a malloc'd buffer
    /// that must be freed with WebPFree(). Returns NULL on failure.
    fn WebPDecodeRGB(
        data: *const u8,
        data_size: usize,
        width: *mut i32,
        height: *mut i32,
    ) -> *mut u8;

    /// Decode WebP image to RGBA (4 bytes/pixel). Returns a malloc'd buffer
    /// that must be freed with WebPFree(). Returns NULL on failure.
    fn WebPDecodeRGBA(
        data: *const u8,
        data_size: usize,
        width: *mut i32,
        height: *mut i32,
    ) -> *mut u8;

    /// Free a buffer allocated by libwebp.
    fn WebPFree(ptr: *mut u8);
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// Decode a WebP image from raw bytes using libwebp.
///
/// Returns RGB (3 bytes/pixel) for images without alpha, RGBA (4 bytes/pixel)
/// for images with alpha. This matches PIL's `tobytes()` output because both
/// use the same libwebp C library internally.
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let data_ptr = data.as_ptr();
    let data_len = data.len();

    // First, get image dimensions
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    let ret = unsafe { WebPGetInfo(data_ptr, data_len, &mut w, &mut h) };
    if ret == 0 || w <= 0 || h <= 0 {
        return None;
    }
    let (width, height) = (w as u32, h as u32);

    // Try to decode as RGB first (most common for our test images)
    // WebPDecodeRGB returns NULL if there's an error or if the image requires
    // alpha-aware decoding. If it succeeds, we get 3 bytes/pixel.
    let mut out_w: i32 = 0;
    let mut out_h: i32 = 0;

    let rgb_ptr = unsafe { WebPDecodeRGB(data_ptr, data_len, &mut out_w, &mut out_h) };
    if !rgb_ptr.is_null() {
        let pixel_count = (out_w as usize) * (out_h as usize);
        let buf = unsafe { std::slice::from_raw_parts(rgb_ptr, pixel_count * 3) }.to_vec();
        unsafe { WebPFree(rgb_ptr) };
        return Some(DecodedImage::new(width, height, buf, ColorType::Rgb8));
    }

    // RGB decode failed — try RGBA (image might have alpha)
    let rgba_ptr = unsafe { WebPDecodeRGBA(data_ptr, data_len, &mut out_w, &mut out_h) };
    if !rgba_ptr.is_null() {
        let pixel_count = (out_w as usize) * (out_h as usize);
        let buf = unsafe { std::slice::from_raw_parts(rgba_ptr, pixel_count * 4) }.to_vec();
        unsafe { WebPFree(rgba_ptr) };
        return Some(DecodedImage::new(width, height, buf, ColorType::Rgba8));
    }

    None
}
