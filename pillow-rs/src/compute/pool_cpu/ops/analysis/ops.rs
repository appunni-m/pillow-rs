//! Direct analysis and GetData support contracts with prepared CPU selection.
//! Analysis mode support is declared by exact-mode owners; this module aggregates
//! declarations and orchestrates prepared operations.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeRoute, OperationSupport,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::helpers::bbox_execution::prepare as prepare_get_bbox;
pub(crate) use super::helpers::extrema_bytes_execution::prepare as prepare_get_extrema_bytes;
pub(crate) use super::helpers::extrema_execution::prepare as prepare_get_extrema;
pub(crate) use super::helpers::getcolors_execution::prepare_getcolors;
pub(crate) use super::helpers::stat_execution::prepare_stat;

#[path = "ops/get_bbox.rs"]
mod get_bbox;
pub(crate) use get_bbox::GET_BBOX_SUPPORT;

#[path = "ops/get_projection.rs"]
mod get_projection;
pub(crate) use get_projection::{
    ProjectionOutput, SUPPORT, execute_cmyk8_selected, execute_rgb8_selected, prepare,
};

const NO_INPUTS: &[InputContract] = &[];
const HISTOGRAM_MASK_MODES: &[Mode] = &[Mode::One, Mode::ImageLuma8];
const HISTOGRAM_EXTREMA_MODES: &[Mode] = &[Mode::ImageI32, Mode::ImageF32];
const HISTOGRAM_MASK_INPUT: InputContract = InputContract::HistogramMask {
    modes: HISTOGRAM_MASK_MODES,
    mode_rejection: "bad transparency mask",
    size_rejection: "images do not match",
};
pub(super) const ANALYSIS_MASK_INPUTS: &[InputContract] = &[HISTOGRAM_MASK_INPUT];
pub(super) const HISTOGRAM_INPUTS: &[InputContract] = &[
    HISTOGRAM_MASK_INPUT,
    InputContract::HistogramExtrema {
        modes: HISTOGRAM_EXTREMA_MODES,
    },
];

fn classify_get_pixel_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::GET_PIXEL_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::GET_PIXEL_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::GET_PIXEL_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::GET_PIXEL_SUPPORT,
        Mode::ImageI32 => super::i32::ops::GET_PIXEL_SUPPORT,
        Mode::ImageF32 => super::f32::ops::GET_PIXEL_SUPPORT,
        Mode::ImageI16 => super::i16::ops::GET_PIXEL_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::GET_PIXEL_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::GET_PIXEL_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::GET_PIXEL_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::GET_PIXEL_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::GET_PIXEL_SUPPORT,
    }
}

/// TODO(pillow-rs#3ptb): verify all 25 typed result shapes and Default receipts in CPU parity.
pub(crate) static GET_PIXEL_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetPixel, NO_INPUTS, classify_get_pixel_mode);

fn classify_get_pixel_rgba_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageI32 => super::i32::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageF32 => super::f32::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageI16 => super::i16::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::GET_PIXEL_RGBA_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::GET_PIXEL_RGBA_SUPPORT,
    }
}

/// TODO(pillow-rs#3ptb): verify fixed-RGBA8 extension output across all exact modes.
pub(crate) static GET_PIXEL_RGBA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetPixelRgba,
    NO_INPUTS,
    classify_get_pixel_rgba_mode,
);

fn classify_get_extrema_bytes_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageI32 => super::i32::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageF32 => super::f32::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageI16 => super::i16::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::GET_EXTREMA_BYTES_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::GET_EXTREMA_BYTES_SUPPORT,
    }
}

/// Exhaustive mode support for byte-valued Rust/JS compatibility extrema.
/// TODO(pillow-rs#3ptb): freeze each mode's byte-result contract and Default
/// fallback receipt for every non-Default Architecture request in CPU parity.
pub(crate) static GET_EXTREMA_BYTES_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetExtremaBytes,
    NO_INPUTS,
    classify_get_extrema_bytes_mode,
);

fn classify_histogram_entropy_mode_support(mode: Mode) -> (ModeSupport, ModeSupport) {
    match mode {
        Mode::One => super::one::ops::histogram_entropy_support(),
        Mode::ImageLuma8 => super::luma8::ops::histogram_entropy_support(),
        Mode::ImageLumaA8 => super::luma_a8::ops::histogram_entropy_support(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::histogram_entropy_support(),
        Mode::ImagePalette8 => super::palette8::ops::histogram_entropy_support(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::histogram_entropy_support(),
        Mode::ImageRgb8 => super::rgb8::ops::histogram_entropy_support(),
        Mode::ImageRgba8 => super::rgba8::ops::histogram_entropy_support(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::histogram_entropy_support(),
        Mode::ImageRgbx8 => super::rgbx8::ops::histogram_entropy_support(),
        Mode::ImageCmyk8 => super::cmyk8::ops::histogram_entropy_support(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::histogram_entropy_support(),
        Mode::ImageLab8 => super::lab8::ops::histogram_entropy_support(),
        Mode::ImageHsv8 => super::hsv8::ops::histogram_entropy_support(),
        Mode::ImageI32 => super::i32::ops::histogram_entropy_support(),
        Mode::ImageF32 => super::f32::ops::histogram_entropy_support(),
        Mode::ImageI16 => super::i16::ops::histogram_entropy_support(),
        Mode::ImageI16L => super::i16l::ops::histogram_entropy_support(),
        Mode::ImageI16B => super::i16b::ops::histogram_entropy_support(),
        Mode::ImageI16N => super::i16n::ops::histogram_entropy_support(),
        Mode::ImageLumaA16 => super::luma_a16::ops::histogram_entropy_support(),
        Mode::ImageRgb16 => super::rgb16::ops::histogram_entropy_support(),
        Mode::ImageRgba16 => super::rgba16::ops::histogram_entropy_support(),
        Mode::ImageRgb32F => super::rgb32f::ops::histogram_entropy_support(),
        Mode::ImageRgba32F => super::rgba32f::ops::histogram_entropy_support(),
    }
}

fn classify_histogram_mode(mode: Mode) -> ModeSupport {
    classify_histogram_entropy_mode_support(mode).0
}

fn classify_entropy_mode(mode: Mode) -> ModeSupport {
    classify_histogram_entropy_mode_support(mode).1
}

/// Exhaustive Histogram contract, including mask inputs and its per-mode
/// result band/bin layout.
pub(crate) static HISTOGRAM_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Histogram,
    HISTOGRAM_INPUTS,
    classify_histogram_mode,
);

/// Exhaustive Entropy support; the implementation composes the Histogram child.
pub(crate) static ENTROPY_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Entropy,
    ANALYSIS_MASK_INPUTS,
    classify_entropy_mode,
    &[OperationId::Histogram],
);

fn classify_getcolors_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::GET_COLORS_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::GET_COLORS_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::GET_COLORS_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::GET_COLORS_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::GET_COLORS_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::GET_COLORS_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::GET_COLORS_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::GET_COLORS_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::GET_COLORS_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::GET_COLORS_SUPPORT,
        Mode::ImageI32 => super::i32::ops::GET_COLORS_SUPPORT,
        Mode::ImageF32 => super::f32::ops::GET_COLORS_SUPPORT,
        Mode::ImageI16 => super::i16::ops::GET_COLORS_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::GET_COLORS_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::GET_COLORS_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::GET_COLORS_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::GET_COLORS_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::GET_COLORS_SUPPORT,
    }
}

/// Exhaustive GetColors mode contract, including Pillow's I;16 rejection.
/// TODO(pillow-rs#3ptb): freeze CPU parity and terminal-receipt cases for the
/// 25-mode × Architecture matrix before marking this operation complete.
pub(crate) static GET_COLORS_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetColors, NO_INPUTS, classify_getcolors_mode);

fn classify_stat_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::stat_mode_support(),
        Mode::ImageLuma8 => super::luma8::ops::stat_mode_support(),
        Mode::ImageLumaA8 => super::luma_a8::ops::stat_mode_support(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::stat_mode_support(),
        Mode::ImagePalette8 => super::palette8::ops::stat_mode_support(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::stat_mode_support(),
        Mode::ImageRgb8 => super::rgb8::ops::stat_mode_support(),
        Mode::ImageRgba8 => super::rgba8::ops::stat_mode_support(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::stat_mode_support(),
        Mode::ImageRgbx8 => super::rgbx8::ops::stat_mode_support(),
        Mode::ImageCmyk8 => super::cmyk8::ops::stat_mode_support(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::stat_mode_support(),
        Mode::ImageLab8 => super::lab8::ops::stat_mode_support(),
        Mode::ImageHsv8 => super::hsv8::ops::stat_mode_support(),
        Mode::ImageI32 => super::i32::ops::stat_mode_support(),
        Mode::ImageF32 => super::f32::ops::stat_mode_support(),
        Mode::ImageI16 => super::i16::ops::stat_mode_support(),
        Mode::ImageI16L => super::i16l::ops::stat_mode_support(),
        Mode::ImageI16B => super::i16b::ops::stat_mode_support(),
        Mode::ImageI16N => super::i16n::ops::stat_mode_support(),
        Mode::ImageLumaA16 => super::luma_a16::ops::stat_mode_support(),
        Mode::ImageRgb16 => super::rgb16::ops::stat_mode_support(),
        Mode::ImageRgba16 => super::rgba16::ops::stat_mode_support(),
        Mode::ImageRgb32F => super::rgb32f::ops::stat_mode_support(),
        Mode::ImageRgba32F => super::rgba32f::ops::stat_mode_support(),
    }
}

/// Exhaustive Stat support; non-scalar modes compose the Histogram child.
/// TODO(pillow-rs#3ptb): freeze all mode/architecture and lazy terminal cases.
pub(crate) static STAT_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Stat,
    ANALYSIS_MASK_INPUTS,
    classify_stat_mode,
    &[OperationId::Histogram],
);

fn classify_split_route(mode: Mode) -> OperationModeRoute {
    match mode {
        Mode::One => super::one::ops::split_mode_route(),
        Mode::ImageLuma8 => super::luma8::ops::split_mode_route(),
        Mode::ImageLumaA8 => super::luma_a8::ops::split_mode_route(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::split_mode_route(),
        Mode::ImagePalette8 => super::palette8::ops::split_mode_route(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::split_mode_route(),
        Mode::ImageRgb8 => super::rgb8::ops::split_mode_route(),
        Mode::ImageRgba8 => super::rgba8::ops::split_mode_route(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::split_mode_route(),
        Mode::ImageRgbx8 => super::rgbx8::ops::split_mode_route(),
        Mode::ImageCmyk8 => super::cmyk8::ops::split_mode_route(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::split_mode_route(),
        Mode::ImageLab8 => super::lab8::ops::split_mode_route(),
        Mode::ImageHsv8 => super::hsv8::ops::split_mode_route(),
        Mode::ImageI32 => super::i32::ops::split_mode_route(),
        Mode::ImageF32 => super::f32::ops::split_mode_route(),
        Mode::ImageI16 => super::i16::ops::split_mode_route(),
        Mode::ImageI16L => super::i16l::ops::split_mode_route(),
        Mode::ImageI16B => super::i16b::ops::split_mode_route(),
        Mode::ImageI16N => super::i16n::ops::split_mode_route(),
        Mode::ImageLumaA16 => super::luma_a16::ops::split_mode_route(),
        Mode::ImageRgb16 => super::rgb16::ops::split_mode_route(),
        Mode::ImageRgba16 => super::rgba16::ops::split_mode_route(),
        Mode::ImageRgb32F => super::rgb32f::ops::split_mode_route(),
        Mode::ImageRgba32F => super::rgba32f::ops::split_mode_route(),
    }
}

/// Exhaustive Split support; each mode exposes its exact copy or child route.
pub(crate) static SPLIT_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::Split,
    NO_INPUTS,
    classify_split_route,
    &[OperationId::ExtractBand],
);

fn classify_get_extrema_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageI32 => super::i32::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageF32 => super::f32::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageI16 => super::i16::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::GET_EXTREMA_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::GET_EXTREMA_SUPPORT,
    }
}

/// Exhaustive native-mode extrema support and architecture fallback contract.
pub(crate) static GET_EXTREMA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::GetExtrema,
    NO_INPUTS,
    classify_get_extrema_mode,
);

fn classify_get_channel_route(mode: Mode) -> OperationModeRoute {
    match mode {
        Mode::One => super::one::ops::get_channel_mode_route(),
        Mode::ImageLuma8 => super::luma8::ops::get_channel_mode_route(),
        Mode::ImageLumaA8 => super::luma_a8::ops::get_channel_mode_route(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::get_channel_mode_route(),
        Mode::ImagePalette8 => super::palette8::ops::get_channel_mode_route(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::get_channel_mode_route(),
        Mode::ImageRgb8 => super::rgb8::ops::get_channel_mode_route(),
        Mode::ImageRgba8 => super::rgba8::ops::get_channel_mode_route(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::get_channel_mode_route(),
        Mode::ImageRgbx8 => super::rgbx8::ops::get_channel_mode_route(),
        Mode::ImageCmyk8 => super::cmyk8::ops::get_channel_mode_route(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::get_channel_mode_route(),
        Mode::ImageLab8 => super::lab8::ops::get_channel_mode_route(),
        Mode::ImageHsv8 => super::hsv8::ops::get_channel_mode_route(),
        Mode::ImageI32 => super::i32::ops::get_channel_mode_route(),
        Mode::ImageF32 => super::f32::ops::get_channel_mode_route(),
        Mode::ImageI16 => super::i16::ops::get_channel_mode_route(),
        Mode::ImageI16L => super::i16l::ops::get_channel_mode_route(),
        Mode::ImageI16B => super::i16b::ops::get_channel_mode_route(),
        Mode::ImageI16N => super::i16n::ops::get_channel_mode_route(),
        Mode::ImageLumaA16 => super::luma_a16::ops::get_channel_mode_route(),
        Mode::ImageRgb16 => super::rgb16::ops::get_channel_mode_route(),
        Mode::ImageRgba16 => super::rgba16::ops::get_channel_mode_route(),
        Mode::ImageRgb32F => super::rgb32f::ops::get_channel_mode_route(),
        Mode::ImageRgba32F => super::rgba32f::ops::get_channel_mode_route(),
    }
}

/// Exhaustive GetChannel contract aggregated from exact-mode owners.
/// TODO(pillow-rs#3ptb): verify all 25 mode outputs, wrapper routes, and
/// selected/fallback receipts in CPU parity.
pub(crate) static GET_CHANNEL_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::GetChannel,
    NO_INPUTS,
    classify_get_channel_route,
    &[OperationId::ExtractBand],
);

fn classify_get_data_support(mode: Mode) -> (ModeSupport, ModeSupport, ModeSupport) {
    match mode {
        Mode::One => super::one::ops::get_data_mode_support(),
        Mode::ImageLuma8 => super::luma8::ops::get_data_mode_support(),
        Mode::ImageLumaA8 => super::luma_a8::ops::get_data_mode_support(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::get_data_mode_support(),
        Mode::ImagePalette8 => super::palette8::ops::get_data_mode_support(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::get_data_mode_support(),
        Mode::ImageRgb8 => super::rgb8::ops::get_data_mode_support(),
        Mode::ImageRgba8 => super::rgba8::ops::get_data_mode_support(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::get_data_mode_support(),
        Mode::ImageRgbx8 => super::rgbx8::ops::get_data_mode_support(),
        Mode::ImageCmyk8 => super::cmyk8::ops::get_data_mode_support(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::get_data_mode_support(),
        Mode::ImageLab8 => super::lab8::ops::get_data_mode_support(),
        Mode::ImageHsv8 => super::hsv8::ops::get_data_mode_support(),
        Mode::ImageI32 => super::i32::ops::get_data_mode_support(),
        Mode::ImageF32 => super::f32::ops::get_data_mode_support(),
        Mode::ImageI16 => super::i16::ops::get_data_mode_support(),
        Mode::ImageI16L => super::i16l::ops::get_data_mode_support(),
        Mode::ImageI16B => super::i16b::ops::get_data_mode_support(),
        Mode::ImageI16N => super::i16n::ops::get_data_mode_support(),
        Mode::ImageLumaA16 => super::luma_a16::ops::get_data_mode_support(),
        Mode::ImageRgb16 => super::rgb16::ops::get_data_mode_support(),
        Mode::ImageRgba16 => super::rgba16::ops::get_data_mode_support(),
        Mode::ImageRgb32F => super::rgb32f::ops::get_data_mode_support(),
        Mode::ImageRgba32F => super::rgba32f::ops::get_data_mode_support(),
    }
}

fn classify_get_data_mode(mode: Mode) -> ModeSupport {
    classify_get_data_support(mode).0
}

/// Exhaustive support declaration for the direct full-buffer GetData read.
/// TODO(pillow-rs#3ptb): verify typed byte lengths and direct view/copy receipts.
pub(crate) static GET_DATA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetData, NO_INPUTS, classify_get_data_mode);

fn classify_get_data_band_mode(mode: Mode) -> ModeSupport {
    classify_get_data_support(mode).1
}

/// Allocated band reads compose caller-buffer and ExtractBand strategies where eligible.
/// TODO(pillow-rs#3ptb): verify mode-specific band counts and lazy child receipts.
pub(crate) static GET_DATA_BAND_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::GetDataBand,
    NO_INPUTS,
    classify_get_data_band_mode,
    &[OperationId::GetDataBandInto, OperationId::ExtractBand],
);

fn classify_get_data_band_into_mode(mode: Mode) -> ModeSupport {
    classify_get_data_support(mode).2
}

/// Caller-owned band reads declare the existing scalar-mode rejection contract.
/// TODO(pillow-rs#3ptb): verify rejection order and output-buffer mutation receipts.
pub(crate) static GET_DATA_BAND_INTO_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::GetDataBandInto,
    NO_INPUTS,
    classify_get_data_band_into_mode,
    &[OperationId::ExtractBand],
);

/// Selects the exact-mode, serial formatted pixel reader.
pub(crate) fn select_get_pixel(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_PIXEL_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the exact-mode, serial RGBA8 convenience reader.
pub(crate) fn select_get_pixel_rgba(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_PIXEL_RGBA_SUPPORT.for_mode(mode).select_serial(context)
}

/// Executes Pillow-shaped GetPixel after its declared exact-mode selection.
pub(crate) fn execute_get_pixel(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<crate::image::FormattedPixelValue, PilError> {
    match image {
        DynamicImage::One(image) => super::one::ops::execute_get_pixel(image, x, y, selection),
        DynamicImage::ImageLuma8(image) => {
            super::luma8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageLumaA8(image) => {
            super::luma_a8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::la_premultiplied8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImagePalette8(image) => {
            super::palette8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::palette_alpha8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgb8(image) => {
            super::rgb8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgba8(image) => {
            super::rgba8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::rgb_premultiplied8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgbx8(image) => {
            super::rgbx8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageCmyk8(image) => {
            super::cmyk8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageYCbCr8(image) => {
            super::ycbcr8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageLab8(image) => {
            super::lab8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageHsv8(image) => {
            super::hsv8::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageI32(image) => super::i32::ops::execute_get_pixel(image, x, y, selection),
        DynamicImage::ImageF32(image) => super::f32::ops::execute_get_pixel(image, x, y, selection),
        DynamicImage::ImageI16(image) => super::i16::ops::execute_get_pixel(image, x, y, selection),
        DynamicImage::ImageI16L(image) => {
            super::i16l::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageI16B(image) => {
            super::i16b::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageI16N(image) => {
            super::i16n::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageLumaA16(image) => {
            super::luma_a16::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgb16(image) => {
            super::rgb16::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgba16(image) => {
            super::rgba16::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgb32F(image) => {
            super::rgb32f::ops::execute_get_pixel(image, x, y, selection)
        }
        DynamicImage::ImageRgba32F(image) => {
            super::rgba32f::ops::execute_get_pixel(image, x, y, selection)
        }
    }
}

/// Executes the allocation-free RGB binding path under the GetPixel declaration.
pub(crate) fn execute_get_pixel_rgb(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<[u8; 3], PilError> {
    let DynamicImage::ImageRgb8(image) = image else {
        return Err(PilError::InternalError(
            "RGB GetPixel selection carried a non-RGB raster".into(),
        ));
    };
    super::rgb8::ops::execute_get_pixel_rgb(image, x, y, selection)
}

/// Executes the Rust/JavaScript fixed-RGBA8 pixel accessor.
pub(crate) fn execute_get_pixel_rgba(
    image: &DynamicImage,
    x: u32,
    y: u32,
    selection: StrategySelection,
) -> Result<[u8; 4], PilError> {
    super::helpers::pixel_access::validate_default_selection(
        selection,
        OperationId::GetPixelRgba,
        image.mode(),
    )?;
    Ok(super::helpers::pixel_access::rgba_pixel(image, x, y))
}

/// Selects the direct full-buffer read for the mode carried by the raster.
pub(crate) fn select_get_data(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the allocated band-read strategy for the exact raster mode.
pub(crate) fn select_get_data_band(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the caller-owned band-read CPU strategy for the exact raster mode.
pub(crate) fn select_get_data_band_into(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_INTO_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

#[path = "helpers/get_data_execution.rs"]
mod get_data_execution;
pub(crate) use get_data_execution::{
    get_data, get_data_band, get_data_band_byte_view, get_data_band_into, get_data_band_into_count,
    get_data_integer_samples, get_data_palette_indices_band_into, get_data_palette_indices_view,
    get_data_scalar_samples, supports_get_data_band_into, supports_get_data_band_terminal,
};
