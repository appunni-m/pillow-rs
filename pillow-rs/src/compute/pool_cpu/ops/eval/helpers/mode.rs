//! Pillow-visible band counts derived from exact mode identities.

use crate::raster::Mode;

pub(crate) fn pillow_band_count(mode: Mode) -> usize {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 | Mode::ImageI32
        | Mode::ImageF32 | Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B
        | Mode::ImageI16N => 1,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => 3,
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => 4,
        // Internal high-depth identities have no Pillow getbands() contract.
        Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16
        | Mode::ImageRgb32F | Mode::ImageRgba32F => 0,
    }
}
