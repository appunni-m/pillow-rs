//! Scalar unpremultiplication shared by native premultiplied-alpha strategies.

/// Removes byte alpha premultiplication using Pillow's `rgba2rgbA` arithmetic.
#[inline]
pub(crate) fn unpremultiply_byte(value: u8, alpha: u8) -> u8 {
    if alpha == 0 || alpha == u8::MAX {
        value
    } else {
        ((u32::from(value) * 255 / u32::from(alpha)).min(255)) as u8
    }
}
