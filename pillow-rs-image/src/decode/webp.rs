//! WebP decoder — uses libwebp C library for pixel-perfect PIL parity.
//!
//! Both PIL and this decoder use the same libwebp code under the hood, so
//! the output is guaranteed to be byte-identical. This is critical for the
//! decode fixture tests which require exact binary equality with PIL.
//!
//! Alpha detection: libwebp's bitstream header MAY report alpha for VP8L
//! images even when every pixel is fully opaque (the VP8L alpha hint is a
//! "may have alpha" flag). To match PIL, we always decode RGBA, then strip
//! the alpha channel when all alpha bytes are 0xFF — this gives us RGB for
//! opaque images and RGBA for genuinely transparent ones.

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
/// for images with alpha. The decoder always decodes RGBA first, then strips
/// the alpha channel to RGB when every alpha byte is 0xFF. This matches PIL's
/// behaviour which uses the same libwebp C library internally.
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

    // Always decode as RGBA first. libwebp's VP8L alpha hint is a "may
    // use alpha" flag — it can be set even when every pixel is opaque.
    // To match PIL we need the actual alpha data.
    let mut out_w: i32 = 0;
    let mut out_h: i32 = 0;
    let rgba_ptr = unsafe { WebPDecodeRGBA(data_ptr, data_len, &mut out_w, &mut out_h) };
    if rgba_ptr.is_null() {
        return None;
    }

    let pixel_count = (out_w as usize) * (out_h as usize);
    let buf = unsafe { std::slice::from_raw_parts(rgba_ptr, pixel_count * 4) }.to_vec();
    unsafe { WebPFree(rgba_ptr) };

    // Check whether any alpha byte is non-0xFF.
    let has_transparency = buf.iter().skip(3).step_by(4).any(|&a| a != 0xFF);

    if has_transparency {
        Some(DecodedImage::new(width, height, buf, ColorType::Rgba8))
    } else {
        // Strip alpha channel to produce 3-byte RGB
        let rgb: Vec<u8> = buf.chunks_exact(4).flat_map(|c| vec![c[0], c[1], c[2]]).collect();
        Some(DecodedImage::new(width, height, rgb, ColorType::Rgb8))
    }
}
