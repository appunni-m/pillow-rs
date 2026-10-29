//! Shared native-byte neighborhood guards and vector kernels.

use crate::raster::DynamicImage;
use wide::{i16x8, i32x8, u8x16, u32x16};

pub(super) const SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS: usize = 64 * 64;
pub(super) const SIMD_ZERO_NEIGHBORHOOD_MAX_PIXELS: usize = 256 * 256;

/// Detect a bounded all-zero native-byte image before a neighborhood pass.
///
/// A zero image is preserved exactly by replicated-edge extrema and by
/// reduction in every admitted byte layout, including premultiplied alpha.
/// Keep this larger scan separate from the general uniform-image check: the
/// zero-byte comparison is cheap and covers the standard 256x256 benchmark,
/// while arbitrary uniform tuples still use the smaller guard below.
pub(super) fn native_zero_byte_image(img: &DynamicImage, channels: usize) -> bool {
    let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
    pixels != 0
        && pixels <= SIMD_ZERO_NEIGHBORHOOD_MAX_PIXELS
        && img.as_bytes().len() == pixels.saturating_mul(channels)
        && img.as_bytes().iter().all(|&byte| byte == 0)
}

/// Detect a constant native-byte image before allocating neighborhood buffers.
///
/// Replicated-edge blur and extrema filters preserve a constant channel tuple
/// exactly. Limit the scan to small images, where avoiding the kernel work is
/// cheaper than adding a full read pass to large non-uniform frames.
pub(super) fn native_small_uniform_byte_image(img: &DynamicImage, channels: usize) -> bool {
    native_uniform_byte_image_up_to(img, channels, SIMD_UNIFORM_NEIGHBORHOOD_MAX_PIXELS)
}

/// Detect a constant native-byte image with an operation-specific bound.
///
/// Convolution can amortize a larger proof scan because a 5x5 pass evaluates
/// twenty-five taps for every interior pixel. Keep the larger bound private to
/// the identity proof so rank/blur paths retain their existing 64x64 guard.
pub(super) fn native_uniform_byte_image_up_to(
    img: &DynamicImage,
    channels: usize,
    max_pixels: usize,
) -> bool {
    let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
    if pixels == 0 || pixels > max_pixels {
        return false;
    }
    let raw = img.as_bytes();
    let mut pixel_chunks = raw.chunks_exact(channels);
    pixel_chunks.next().is_some_and(|first_pixel| {
        pixel_chunks.remainder().is_empty() && pixel_chunks.all(|pixel| pixel == first_pixel)
    })
}

pub(super) const SIMD_BOX_BLUR_SCALE: u32 = 1 << 24;
pub(super) const SIMD_BOX_BLUR_BIAS: u32 = 1 << 23;

#[inline(always)]
pub(super) fn simd_blur_load_u8x16(bytes: &[u8], start: usize) -> u8x16 {
    bytemuck::pod_read_unaligned(&bytes[start..start + 16])
}

#[inline(always)]
pub(super) fn simd_blur_pack_u32x16_to_bytes(values: u32x16) -> [u8; 16] {
    // The rounded blur lanes are in 0..=255, so signed saturating packs are
    // exact and avoid extracting and narrowing every lane in scalar code.
    let [low, high]: [i32x8; 2] = bytemuck::cast(values);
    let low = i16x8::from_i32x8_saturate(low);
    let high = i16x8::from_i32x8_saturate(high);
    bytemuck::cast(u8x16::narrow_i16x8(low, high))
}
