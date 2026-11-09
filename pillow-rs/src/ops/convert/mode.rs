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
    mode.is_pillow_mode()
}
