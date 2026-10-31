//! Bounded exact-byte uniformity checks shared by CPU filter families.

pub(in crate::compute::pool_cpu::ops::filter) const SMALL_UNIFORM_BYTE_MAX_PIXELS: usize =
    64 * 64;
const ZERO_BYTE_MAX_PIXELS: usize = 256 * 256;

/// Return whether a small native-byte image contains one repeated channel tuple.
///
/// Constant neighborhoods remain unchanged by these filters. Keep the scan
/// bounded so the fast path cannot add a full-frame pass to larger inputs.
#[inline]
pub(in crate::compute::pool_cpu::ops::filter) fn native_small_uniform_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    max_pixels: usize,
) -> bool {
    if channels == 0 || width == 0 || height == 0 {
        return false;
    }
    let Some(pixel_count) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    if pixel_count > max_pixels {
        return false;
    }
    let Some(first) = raw.get(..channels) else {
        return false;
    };
    raw.chunks_exact(channels).all(|pixel| pixel == first)
}

/// Return whether a bounded native-byte image is all zero.
#[inline]
pub(in crate::compute::pool_cpu::ops::filter) fn native_zero_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
) -> bool {
    if channels == 0 || width == 0 || height == 0 {
        return false;
    }
    let Some(pixels) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    if pixels > ZERO_BYTE_MAX_PIXELS {
        return false;
    }
    let Some(bytes) = pixels.checked_mul(channels) else {
        return false;
    };
    raw.len() == bytes && raw.iter().all(|&byte| byte == 0)
}

