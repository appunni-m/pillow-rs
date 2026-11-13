//! Per-pixel alpha arithmetic shared by CPU operation families.

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(crate) fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return value;
    }
    // Both inputs are bytes, so Pillow's f64 expression has an exact
    // integer numerator. Integer division preserves its truncation; clamp
    // because Rust's float-to-u8 cast in the reference path saturates.
    (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8
}
