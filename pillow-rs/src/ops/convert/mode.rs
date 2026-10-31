use crate::raster::Mode;

/// Whether this identity uses Pillow's unsigned 16-bit luma carrier.
pub(super) const fn is_luma16_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

/// Modes whose sample storage needs Pillow-specific color or scalar handling.
pub(super) const fn is_nonstandard_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageCmyk8
            | Mode::ImageHsv8
            | Mode::ImageYCbCr8
            | Mode::ImageI32
            | Mode::ImageF32
            | Mode::ImagePalette8
            | Mode::ImagePaletteAlpha8
    )
}

/// Whether Convert currently has a Pillow-defined destination for this mode.
/// Internal high-depth raster identities remain representable but unsupported
/// by this operation until they have an explicit native conversion contract.
pub(super) const fn is_convert_target_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImagePalette8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageRgb8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageYCbCr8
            | Mode::ImageLab8
            | Mode::ImageHsv8
            | Mode::ImageI32
            | Mode::ImageF32
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    )
}

/// Extracts the Y band of a YCbCr image as luma bytes.
///
/// Pillow's C converter (`src/libImaging/Convert.c`) maps YCbCr to L and `1`
/// through the Y channel directly rather than through RGB luminance.
pub(super) fn ycbcr_luma8(img: &crate::raster::DynamicImage) -> crate::raster::GrayImage {
    let rgb = img.to_rgb8();
    crate::raster::GrayImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        crate::raster::Luma([rgb.get_pixel(x, y)[0]])
    })
}
