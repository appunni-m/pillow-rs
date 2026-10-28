//! Bounded exact-byte uniformity checks shared by CPU filter families.

use crate::raster::DynamicImage;

pub(super) const SMALL_UNIFORM_BYTE_MAX_PIXELS: usize = 64 * 64;

/// Return whether a small native-byte image contains one repeated channel tuple.
///
/// Constant neighborhoods remain unchanged by these filters. Keep the scan
/// bounded so the fast path cannot add a full-frame pass to larger inputs.
#[inline]
pub(super) fn native_small_uniform_byte_image(
    img: &DynamicImage,
    channels: usize,
    max_pixels: usize,
) -> bool {
    if channels == 0 || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(pixel_count) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    if pixel_count > max_pixels {
        return false;
    }
    let raw = img.as_bytes();
    let Some(first) = raw.get(..channels) else {
        return false;
    };
    raw.chunks_exact(channels).all(|pixel| pixel == first)
}
