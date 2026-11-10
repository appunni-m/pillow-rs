//! Closed mode strategy for Thumbnail's mode-dependent policy.

use crate::pipeline::ResampleFilter;
use crate::raster::Mode;

#[derive(Clone, Copy)]
pub(super) enum ThumbnailModeStrategy {
    Standard,
    One,
    Palette8,
    LumaA8,
    LaPremultiplied8,
    PaletteAlpha8,
    Rgba8,
    I32,
    F32,
}

impl ThumbnailModeStrategy {
    pub(super) const fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::One => Self::One,
            Mode::ImageLuma8 => Self::Standard,
            Mode::ImageLumaA8 => Self::LumaA8,
            Mode::ImageLaPremultiplied8 => Self::LaPremultiplied8,
            Mode::ImagePalette8 => Self::Palette8,
            Mode::ImagePaletteAlpha8 => Self::PaletteAlpha8,
            Mode::ImageRgb8 => Self::Standard,
            Mode::ImageRgba8 => Self::Rgba8,
            Mode::ImageRgbPremultiplied8 => Self::Standard,
            Mode::ImageRgbx8 => Self::Standard,
            Mode::ImageCmyk8 => Self::Standard,
            Mode::ImageYCbCr8 => Self::Standard,
            Mode::ImageLab8 => Self::Standard,
            Mode::ImageHsv8 => Self::Standard,
            Mode::ImageI32 => Self::I32,
            Mode::ImageF32 => Self::F32,
            Mode::ImageI16 => Self::Standard,
            Mode::ImageI16L => Self::Standard,
            Mode::ImageI16B => Self::Standard,
            Mode::ImageI16N => Self::Standard,
            Mode::ImageLumaA16 => Self::Standard,
            Mode::ImageRgb16 => Self::Standard,
            Mode::ImageRgba16 => Self::Standard,
            Mode::ImageRgb32F => Self::Standard,
            Mode::ImageRgba32F => Self::Standard,
        }
    }

    pub(super) fn effective_filter(self, requested: ResampleFilter) -> ResampleFilter {
        match self {
            Self::One => super::one::ops::effective_filter(requested),
            Self::Palette8 => super::palette8::ops::effective_filter(requested),
            Self::PaletteAlpha8 => super::palette_alpha8::ops::effective_filter(requested),
            Self::Standard
            | Self::LumaA8
            | Self::LaPremultiplied8
            | Self::Rgba8
            | Self::I32
            | Self::F32 => requested,
        }
    }

    pub(super) fn should_reduce(self, filter: ResampleFilter, reducing_gap: Option<f64>) -> bool {
        if reducing_gap.is_none() {
            return false;
        }
        match self {
            Self::LumaA8 => super::luma_a8::ops::should_reduce(filter),
            Self::LaPremultiplied8 => super::la_premultiplied8::ops::should_reduce(filter),
            Self::PaletteAlpha8 => super::palette_alpha8::ops::should_reduce(filter),
            Self::Rgba8 => super::rgba8::ops::should_reduce(filter),
            Self::Standard | Self::One | Self::Palette8 | Self::I32 | Self::F32 => {
                !matches!(filter, ResampleFilter::Nearest)
            }
        }
    }

    pub(super) fn resize_reducing_gap(
        self,
        filter: ResampleFilter,
        reducing_gap: Option<f64>,
    ) -> Option<f64> {
        match self {
            Self::LumaA8 | Self::Rgba8 if !matches!(filter, ResampleFilter::Nearest) => {
                reducing_gap
            }
            Self::Standard
            | Self::One
            | Self::Palette8
            | Self::LumaA8
            | Self::LaPremultiplied8
            | Self::PaletteAlpha8
            | Self::Rgba8
            | Self::I32
            | Self::F32 => None,
        }
    }

    pub(super) fn uses_unboxed_resize(
        self,
        source: (u32, u32),
        output: (u32, u32),
        factors: (u32, u32),
    ) -> bool {
        match self {
            Self::F32 => super::f32::ops::uses_unboxed_resize(source, output, factors),
            Self::Standard
            | Self::One
            | Self::Palette8
            | Self::LumaA8
            | Self::LaPremultiplied8
            | Self::PaletteAlpha8
            | Self::Rgba8
            | Self::I32 => false,
        }
    }

    pub(super) fn streams_i32_thumbnail(
        self,
        source: (u32, u32),
        output: (u32, u32),
        filter: ResampleFilter,
        factors: (u32, u32),
    ) -> bool {
        match self {
            Self::I32 => {
                factors == (2, 2) && super::i32::ops::streams_two_by_two(source, output, filter)
            }
            Self::Standard
            | Self::One
            | Self::Palette8
            | Self::LumaA8
            | Self::LaPremultiplied8
            | Self::PaletteAlpha8
            | Self::Rgba8
            | Self::F32 => false,
        }
    }
}
