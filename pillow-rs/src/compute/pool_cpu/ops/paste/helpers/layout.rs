use crate::raster::Mode;

/// Exact native input mapping chosen once before entering a pixel loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceLayout {
    Native,
    LumaAlphaToRgb,
    FourBytesToRgb,
    RgbToRgba,
}

/// Exact mask storage location and mask arithmetic selected during preparation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MaskLayout {
    pub(crate) channels: usize,
    pub(crate) value_index: usize,
    pub(crate) binary: bool,
    pub(crate) premultiplied: bool,
}

pub(crate) fn source_layout(
    destination: Mode,
    source: Mode,
) -> Option<SourceLayout> {
    if destination == source {
        return Some(SourceLayout::Native);
    }
    match (destination, source) {
        (Mode::ImageRgb8, Mode::ImageLumaA8) => Some(SourceLayout::LumaAlphaToRgb),
        (Mode::ImageRgb8, Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8) => {
            Some(SourceLayout::FourBytesToRgb)
        }
        (Mode::ImageRgba8, Mode::ImageRgb8) => Some(SourceLayout::RgbToRgba),
        _ => None,
    }
}

pub(crate) fn mask_layout(mode: Mode, alpha: bool) -> Option<MaskLayout> {
    match (mode, alpha) {
        (Mode::One, false) => Some(MaskLayout {
            channels: 1,
            value_index: 0,
            binary: true,
            premultiplied: false,
        }),
        (Mode::ImageLuma8, false) => Some(MaskLayout {
            channels: 1,
            value_index: 0,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageLumaA8, true) => Some(MaskLayout {
            channels: 2,
            value_index: 1,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageRgba8, true) => Some(MaskLayout {
            channels: 4,
            value_index: 3,
            binary: false,
            premultiplied: false,
        }),
        (Mode::ImageRgbPremultiplied8, true) => Some(MaskLayout {
            channels: 4,
            value_index: 3,
            binary: false,
            premultiplied: true,
        }),
        _ => None,
    }
}

pub(crate) const fn byte_channels(mode: Mode) -> Option<usize> {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => Some(1),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => Some(2),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => Some(3),
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => Some(4),
        _ => None,
    }
}
