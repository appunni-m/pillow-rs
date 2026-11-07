pub(crate) fn unpremultiply_rgba(rgba: &crate::raster::RgbaImage) -> crate::raster::RgbaImage {
    let mut out = rgba.clone();
    for p in out.pixels_mut() {
        let Some(alpha) = std::num::NonZeroU32::new(u32::from(p[3])) else {
            continue;
        };
        // Pillow's RGBa -> RGBA conversion uses integer
        // `(255 * value) / alpha` truncation. Keep the multiply
        // before division: using `255.0 / alpha` and a floating
        // multiply can land one ULP below an exact integer.
        p[0] = (u32::from(p[0]) * 255 / alpha.get()).min(255) as u8;
        p[1] = (u32::from(p[1]) * 255 / alpha.get()).min(255) as u8;
        p[2] = (u32::from(p[2]) * 255 / alpha.get()).min(255) as u8;
    }
    out
}

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
