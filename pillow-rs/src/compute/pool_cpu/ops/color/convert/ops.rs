//! Convert support contract and mode/architecture orchestration.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationSupport, OutputModeRule,
};
use crate::raster::Mode;

pub(super) const LUMA_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageLab8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(super) const LA_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageLab8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(super) const RGBA_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageLab8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(super) const RGB_PREMULTIPLIED_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgb8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgba8,
    Mode::ImageLab8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const LA_PREMULTIPLIED_TARGETS: &[Mode] = &[
    Mode::ImageLaPremultiplied8,
    Mode::ImageLumaA8,
];
pub(super) const I16_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageLab8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(crate) const RGBX_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageLab8,
    Mode::ImageI32,
    Mode::ImageF32,
];
pub(super) const SCALAR_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageLab8,
    Mode::ImageI32,
    Mode::ImageF32,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
];
const MATRIX_TARGETS: &[Mode] = &[Mode::ImageLuma8, Mode::ImageRgb8];
const MATRIX_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: MATRIX_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const LA_PREMULTIPLIED_INPUTS: &[InputContract] =
    &[InputContract::ConvertTarget {
        targets: LA_PREMULTIPLIED_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "conversion from La to L not supported",
    }];
pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SUPPORT,
        Mode::ImageI32 => super::i32::ops::SUPPORT,
        Mode::ImageF32 => super::f32::ops::SUPPORT,
        Mode::ImageI16 => super::i16::ops::SUPPORT,
        Mode::ImageI16L => super::i16l::ops::SUPPORT,
        Mode::ImageI16B => super::i16b::ops::SUPPORT,
        Mode::ImageI16N => super::i16n::ops::SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
    }
}

/// Complete source-mode and typed-target contract for Convert.
///
/// Convert-to-LAB is a wrapper composition: each source mode selects its
/// declared RGB normalization route, then the RGB8 ConvertLab operation runs.
// Archive source review: SIMD's generic byte-layout preflight admits L, LA,
// RGB, and RGBA carriers with their explicit Pillow aliases; RGBa alpha
// transforms, CMYK→RGB, I;16 conversions, and One/L→I;16 use separate paths.
// The maintained One architecture guards match the archived four I;16 targets.
// Source review now confirms the One and L source/target routes, their exact
// architecture sets and build guards, normalized One samples, and empty-path
// receipts against the archived adapter. L same-mode output remains the
// Default identity path; palette and LAB targets use their declared wrapper
// compositions. The exact LA target contract, alpha transformations, vector
// targets, and empty-path receipts are source-reviewed as well. RGB8's 20
// targets, 38 architecture guards, and Default/composed branches now reconcile
// with the archived luma, alpha, color-space, typed, palette, binary, and LAB
// paths; its same-mode target stays on Default, with AVX2/SSSE3 limited to L.
// Source review compares all 25 source modes against the archived Convert
// families; the per-source target and architecture maps are recorded in the
// CPU refactor inventory. Keep runtime parity, selection/fallback receipts,
// and target build availability pending until the CPU verification gate.
// Same-mode routes use each source mode's Default identity path. P/PA
// conversion also produces wrapper-owned palette/alpha metadata. The public
// Image boundary and its dedicated lazy materialization path publish those
// sidecars; the generic raster-only prepared entry still needs a result type
// that carries them. Internal high-depth identities are CoreOnly.
const CONVERT_CHILDREN: &[OperationId] = &[OperationId::ConvertLab];

pub(crate) static SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Convert,
    &[],
    classify_mode,
    CONVERT_CHILDREN,
)
.with_rejected_output(OutputModeRule::ByTargetMode)
.with_target_compositions(super::helpers::target_compositions::classify_target_composition);

fn classify_matrix_mode(mode: Mode) -> ModeSupport {
    if mode == Mode::ImageRgb8 {
        return ModeSupport::cpu(
            OperationId::ConvertMatrix,
            mode,
            crate::compute::registry::support::OutputModeRule::ByTargetMode,
            MATRIX_INPUTS,
            &[],
        );
    }

    // NO_PILLOW_SUPPORT: native samples preserve the existing RGB projection
    // semantics for these exact source identities.
    ModeSupport::cpu_core_only(
        OperationId::ConvertMatrix,
        mode,
        crate::compute::registry::support::OutputModeRule::ByTargetMode,
        MATRIX_INPUTS,
        &[],
        "matrix conversion from this exact source identity extends Pillow's RGB matrix contract",
    )
}

/// Matrix conversion support, with one Default strategy for each exact source mode.
pub(crate) static MATRIX_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ConvertMatrix,
    &[],
    classify_matrix_mode,
)
.with_rejected_output(OutputModeRule::ByTargetMode);

pub(crate) use super::helpers::prepared::{
    prepare, prepare_matrix, prepare_pipeline, ConvertWrapperMetadata, Prepared,
    PreparedConvert, PreparedConvertOutput,
};
