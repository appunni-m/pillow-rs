use crate::compute::registry::support::PasteMaskRule;
use crate::raster::Mode;

pub(crate) const MASKS: &[PasteMaskRule] = &[
    PasteMaskRule {
        mode: Mode::One,
        alpha: false,
    },
    PasteMaskRule {
        mode: Mode::ImageLuma8,
        alpha: false,
    },
    PasteMaskRule {
        mode: Mode::ImageLumaA8,
        alpha: true,
    },
    PasteMaskRule {
        mode: Mode::ImageRgba8,
        alpha: true,
    },
    PasteMaskRule {
        mode: Mode::ImageRgbPremultiplied8,
        alpha: true,
    },
];

pub(crate) const COMPACT_MASK_ONE: &[Mode] = &[Mode::One];
pub(crate) const COMPACT_MASK_ONE_OR_LUMA: &[Mode] = &[Mode::One, Mode::ImageLuma8];
pub(crate) const PHYSICAL_ONE_BYTE_SOURCES: &[Mode] =
    &[Mode::One, Mode::ImageLuma8, Mode::ImagePalette8];
pub(crate) const I16_PIXEL_SIZE_SOURCES: &[Mode] = &[
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
];
pub(crate) const PHYSICAL_FOUR_BYTE_SOURCES: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageLab8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];

pub(crate) const fn physical_byte_sources(mode: Mode) -> &'static [Mode] {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => PHYSICAL_ONE_BYTE_SOURCES,
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
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
        | Mode::ImageF32 => PHYSICAL_FOUR_BYTE_SOURCES,
        _ => same_mode_source(mode),
    }
}

pub(crate) const fn same_mode_source(mode: Mode) -> &'static [Mode] {
    match mode {
        Mode::One => &[Mode::One],
        Mode::ImageLuma8 => &[Mode::ImageLuma8],
        Mode::ImageLumaA8 => &[Mode::ImageLumaA8],
        Mode::ImageLaPremultiplied8 => &[Mode::ImageLaPremultiplied8],
        Mode::ImagePalette8 => &[Mode::ImagePalette8],
        Mode::ImagePaletteAlpha8 => &[Mode::ImagePaletteAlpha8],
        Mode::ImageRgb8 => &[Mode::ImageRgb8],
        Mode::ImageRgba8 => &[Mode::ImageRgba8],
        Mode::ImageRgbPremultiplied8 => &[Mode::ImageRgbPremultiplied8],
        Mode::ImageRgbx8 => &[Mode::ImageRgbx8],
        Mode::ImageCmyk8 => &[Mode::ImageCmyk8],
        Mode::ImageYCbCr8 => &[Mode::ImageYCbCr8],
        Mode::ImageLab8 => &[Mode::ImageLab8],
        Mode::ImageHsv8 => &[Mode::ImageHsv8],
        Mode::ImageI32 => &[Mode::ImageI32],
        Mode::ImageF32 => &[Mode::ImageF32],
        Mode::ImageI16 => &[Mode::ImageI16],
        Mode::ImageI16L => &[Mode::ImageI16L],
        Mode::ImageI16B => &[Mode::ImageI16B],
        Mode::ImageI16N => &[Mode::ImageI16N],
        Mode::ImageLumaA16 => &[Mode::ImageLumaA16],
        Mode::ImageRgb16 => &[Mode::ImageRgb16],
        Mode::ImageRgba16 => &[Mode::ImageRgba16],
        Mode::ImageRgb32F => &[Mode::ImageRgb32F],
        Mode::ImageRgba32F => &[Mode::ImageRgba32F],
    }
}

pub(crate) const fn compact_sources(mode: Mode) -> &'static [Mode] {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => PHYSICAL_ONE_BYTE_SOURCES,
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
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
        | Mode::ImageF32 => PHYSICAL_FOUR_BYTE_SOURCES,
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => &[],
    }
}

pub(crate) const fn compact_masks(mode: Mode) -> &'static [Mode] {
    match mode {
        Mode::One | Mode::ImagePalette8 | Mode::ImageLumaA8 => COMPACT_MASK_ONE,
        Mode::ImageLuma8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageCmyk8
        | Mode::ImageHsv8 => COMPACT_MASK_ONE_OR_LUMA,
        _ => &[],
    }
}
