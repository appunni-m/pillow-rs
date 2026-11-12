//! Exact-mode support classification shared by native byte point operations.

use crate::compute::registry::support::{ModeSupport, RejectionKind};
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn classify_native_byte_point_mode(
    mode: Mode,
    luma_support: ModeSupport,
    palette_support: ModeSupport,
    rgb_support: ModeSupport,
) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => luma_support,
        Mode::ImageRgb8 => rgb_support,
        Mode::ImagePalette8 => palette_support,
        Mode::One
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
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
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => rejected_native_byte_point_mode(mode),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn rejected_native_byte_point_mode(mode: Mode) -> ModeSupport {
    let reason = match mode {
        Mode::One => "not supported for mode 1",
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8 => "not supported for this mode",
        Mode::ImageLumaA8 => "not supported for mode LA",
        Mode::ImageLaPremultiplied8 => "not supported for mode La",
        Mode::ImagePaletteAlpha8 => "not supported for mode PA",
        Mode::ImageRgba8 => "not supported for mode RGBA",
        Mode::ImageRgbPremultiplied8 => "not supported for mode RGBa",
        Mode::ImageRgbx8 => "not supported for mode RGBX",
        Mode::ImageCmyk8 => "not supported for mode CMYK",
        Mode::ImageYCbCr8 => "not supported for mode YCbCr",
        Mode::ImageLab8 => "not supported for mode LAB",
        Mode::ImageHsv8 => "not supported for mode HSV",
        Mode::ImageI32 => "not supported for mode I",
        Mode::ImageF32 => "not supported for mode F",
        Mode::ImageI16 => "not supported for mode I;16",
        Mode::ImageI16L => "not supported for mode I;16L",
        Mode::ImageI16B => "not supported for mode I;16B",
        Mode::ImageI16N => "not supported for mode I;16N",
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => super::strategy::unsupported_mode_reason(mode),
    };
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::OsError,
    }
}
