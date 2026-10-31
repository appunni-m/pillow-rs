//! Typed byte-payload properties shared by PutData boundaries and executors.

use crate::raster::Mode;

/// Raw bytes per normalized PutData payload pixel for supported byte carriers.
///
/// The `Mode` is the exact payload identity, not an inferred source image mode.
/// `None` preserves the existing rejection of modes without a byte payload
/// route, including typed high-depth samples.
pub(crate) const fn payload_bytes_per_pixel(mode: Mode) -> Option<usize> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => Some(1),
        Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 => Some(2),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageHsv8 => Some(3),
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => Some(4),
        Mode::ImageLaPremultiplied8
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLab8
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => None,
    }
}
