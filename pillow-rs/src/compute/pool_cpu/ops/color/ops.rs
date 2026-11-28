//! Color operation support declarations and prepared orchestration.

use crate::compute::{
    ExecutionContext,
    registry::support::{
        ConvertMatrixTarget, InputContract, ModeSupport, OperationId, OperationInputs,
        OperationSupport, OutputModeRule, RejectionKind, StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(super) const RGB_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 3,
    overflow_reason: "LAB image size overflow",
    mismatch_reason: "LAB conversion requires tightly packed RGB8 input",
}];

pub(super) const RGBA_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 4,
    overflow_reason: "LAB image size overflow",
    mismatch_reason: "LAB conversion requires tightly packed RGBA8 input",
}];

pub(super) const RGBX_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 4,
    overflow_reason: "LAB image size overflow",
    mismatch_reason: "LAB conversion requires tightly packed RGBX8 input",
}];

pub(super) const TRANSPARENT_RGB_TARGETS: &[Mode] = &[
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
];
pub(super) const TRANSPARENT_GRAY_TARGETS: &[Mode] = &[Mode::ImageRgba8, Mode::ImageLumaA8];
pub(super) const TRANSPARENT_RGB_INPUTS: &[InputContract] = &[
    InputContract::ExactNativeBytes {
        channels: 3,
        overflow_reason: "ConvertTransparent RGB image size overflow",
        mismatch_reason: "ConvertTransparent RGB input has wrong native byte length",
    },
    InputContract::ConvertTarget {
        targets: TRANSPARENT_RGB_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "conversion not supported in convert_transparent",
    },
];
pub(super) const TRANSPARENT_GRAY_BYTE_INPUTS: &[InputContract] = &[
    InputContract::ExactNativeBytes {
        channels: 1,
        overflow_reason: "ConvertTransparent gray image size overflow",
        mismatch_reason: "ConvertTransparent gray input has wrong native byte length",
    },
    InputContract::ConvertTarget {
        targets: TRANSPARENT_GRAY_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "conversion not supported in convert_transparent",
    },
];
pub(super) const TRANSPARENT_I32_INPUTS: &[InputContract] = &[
    InputContract::ExactNativeBytes {
        channels: 4,
        overflow_reason: "ConvertTransparent I image size overflow",
        mismatch_reason: "ConvertTransparent I input has wrong native byte length",
    },
    InputContract::ConvertTarget {
        targets: TRANSPARENT_GRAY_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "conversion not supported in convert_transparent",
    },
];
pub(super) const TRANSPARENT_I16_INPUTS: &[InputContract] = &[
    InputContract::ExactNativeBytes {
        channels: 2,
        overflow_reason: "ConvertTransparent I;16 image size overflow",
        mismatch_reason: "ConvertTransparent I;16 input has wrong native byte length",
    },
    InputContract::ConvertTarget {
        targets: TRANSPARENT_GRAY_TARGETS,
        core_only_targets: &[],
        core_only_reason: None,
        rejection_reason: "conversion not supported in convert_transparent",
    },
];

pub(super) const LUMA_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
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
// RGBa's exact source row in Convert's supported mode-target matrix.
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
    Mode::ImageLab8,
];
const LA_PREMULTIPLIED_CORE_ONLY_TARGETS: &[Mode] = &[Mode::ImageLab8];
const LA_PREMULTIPLIED_CORE_ONLY_REASON: &str =
    "direct native La-to-LAB conversion extends Pillow's La conversion contract";
// TODO(pillow-rs#3ptb): verify every I;16 source/target pair, including the
// native-u16 extensions, across all declared architectures and fault cases.
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
    Mode::ImageRgbx8,
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
// NO_PILLOW_SUPPORT: RGB/4 is deterministic Rust-only behavior; Pillow's
// four-coefficient RGB result does not define the remaining two channel rows.
// A twelve-coefficient RGB matrix may feed LAB without an intermediate image.
const MATRIX_TARGETS: &[ConvertMatrixTarget] = &[
    ConvertMatrixTarget {
        mode: Mode::ImageLuma8,
        coefficients: 4,
        core_only_reason: None,
    },
    ConvertMatrixTarget {
        mode: Mode::ImageLuma8,
        coefficients: 12,
        core_only_reason: None,
    },
    ConvertMatrixTarget {
        mode: Mode::ImageRgb8,
        coefficients: 4,
        core_only_reason: Some(
            "four-coefficient RGB matrix output is a deterministic Rust extension",
        ),
    },
    ConvertMatrixTarget {
        mode: Mode::ImageRgb8,
        coefficients: 12,
        core_only_reason: None,
    },
    ConvertMatrixTarget {
        mode: Mode::ImageLab8,
        coefficients: 12,
        core_only_reason: Some(
            "direct matrix conversion to LAB extends Pillow's RGB matrix contract",
        ),
    },
];
const MATRIX_INPUTS: &[InputContract] = &[InputContract::ConvertMatrixTarget {
    targets: MATRIX_TARGETS,
    length_rejection_reason: "matrix must be tuple of length 4 or 12",
    target_rejection_reason: "image has wrong mode",
    coefficient_rejection_reason: "matrix coefficient count does not match target mode",
}];

pub(crate) const LA_PREMULTIPLIED_INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: LA_PREMULTIPLIED_TARGETS,
    core_only_targets: LA_PREMULTIPLIED_CORE_ONLY_TARGETS,
    core_only_reason: Some(LA_PREMULTIPLIED_CORE_ONLY_REASON),
    rejection_reason: "conversion from La to L not supported",
}];
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::CONVERT_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::CONVERT_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::CONVERT_SUPPORT,
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
        Mode::ImagePalette8 => super::palette8::ops::CONVERT_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::CONVERT_SUPPORT,
    }
}

/// Complete source-mode and typed-target contract for Convert.
// TODO(pillow-rs#3ptb): close the complete source × target × declared
// architecture output/fault matrix before the CPU parity gate.
// Archive source review: SIMD's generic byte-layout preflight admits L, LA,
// RGB, and RGBA carriers with their explicit Pillow aliases; RGBa alpha
// transforms, CMYK→RGB, I;16 conversions, and One/L→I;16 use separate paths.
// The maintained One architecture guards match the archived four I;16 targets.
// Source review now confirms the One and L source/target routes, their exact
// architecture sets and build guards, normalized One samples, and empty-path
// receipts against the archived adapter. L same-mode output remains the
// Default identity path; P/PA targets attach wrapper-owned palette metadata,
// while LAB runs in each exact source-mode Convert strategy. The exact LA
// target contract, alpha transformations, vector targets, and empty-path
// receipts are source-reviewed as well. RGB8's 20 targets, 38 architecture
// guards, and Default/vector branches now reconcile
// with the archived luma, alpha, color-space, typed, palette, binary, and LAB
// paths; its same-mode target stays on Default, with AVX2/SSSE3 limited to L.
// Source review compares all 25 source modes against the archived Convert
// families; the per-source target and architecture maps are recorded in the
// CPU refactor inventory. Keep runtime parity, selection receipts, declared
// Default-architecture fallback receipts, and target build availability
// pending until the CPU verification gate.
// Same-mode routes return the typed source raster from the prepared identity
// path. P/PA conversion produces wrapper-owned palette/alpha metadata; the
// Image boundary publishes those sidecars. Internal high-depth identities
// and cross-high-depth targets are CoreOnly.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) static CONVERT_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Convert, &[], classify_mode)
        .with_rejected_output(OutputModeRule::ByTargetMode)
        .with_target_compositions(super::helpers::convert::classify_target_composition);

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn classify_convert_transparent_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::CONVERT_TRANSPARENT_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::CONVERT_TRANSPARENT_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::CONVERT_TRANSPARENT_SUPPORT,
        Mode::ImageI32 => super::i32::ops::CONVERT_TRANSPARENT_SUPPORT,
        Mode::ImageI16 => super::i16::ops::CONVERT_TRANSPARENT_SUPPORT,
        _ => ModeSupport::Rejected {
            mode,
            reason: "conversion not supported in convert_transparent",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Native mode/target matrix for ImagingCore's transparent conversion.
pub(crate) static CONVERT_TRANSPARENT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ConvertTransparent,
    &[],
    classify_convert_transparent_mode,
)
.with_rejected_output(OutputModeRule::ByTargetMode);

pub(crate) struct PreparedConvertTransparent<'a> {
    source: &'a DynamicImage,
    target: Mode,
    key: [u8; 3],
    selection: StrategySelection,
}

impl PreparedConvertTransparent<'_> {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) const fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.source {
            DynamicImage::One(source) => super::one::ops::execute_transparent(
                source,
                self.target,
                [self.key[0]; 3],
                self.selection,
            ),
            DynamicImage::ImageLuma8(source) => super::luma8::ops::execute_transparent(
                source,
                self.target,
                [self.key[0]; 3],
                self.selection,
            ),
            DynamicImage::ImageRgb8(source) => {
                super::rgb8::ops::execute_transparent(source, self.target, self.key, self.selection)
            }
            DynamicImage::ImageI32(source) => super::i32::ops::execute_transparent(
                source,
                self.target,
                [self.key[0]; 3],
                self.selection,
            ),
            DynamicImage::ImageI16(source) => super::i16::ops::execute_transparent(
                source,
                self.target,
                [self.key[0]; 3],
                self.selection,
            ),
            _ => Err(PilError::InternalError(
                "ConvertTransparent source changed after strategy preparation".into(),
            )),
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn prepare_convert_transparent<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    key: [u8; 3],
    ctx: &ExecutionContext,
) -> Result<PreparedConvertTransparent<'a>, PilError> {
    let source_mode = input.mode();
    let support = CONVERT_TRANSPARENT_SUPPORT.for_mode(source_mode);
    validate_convert_transparent_target(source_mode, target)?;
    let facts = input.facts().with_convert_target(target);
    let selection = match support.assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "ConvertTransparent support remained unresolved".into(),
            ));
        }
    };
    Ok(PreparedConvertTransparent {
        source: input.image,
        target,
        key,
        selection,
    })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn validate_convert_transparent_target(
    source: Mode,
    target: Mode,
) -> Result<(), PilError> {
    if CONVERT_TRANSPARENT_SUPPORT
        .scope_for_target(source, target)
        .is_some()
    {
        return Ok(());
    }
    let source_name = source.pillow_name().unwrap_or("unknown");
    let target_name = target.pillow_name().unwrap_or("unknown");
    Err(PilError::ValueError(format!(
        "conversion from {source_name} to {target_name} not supported in convert_transparent"
    )))
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn classify_matrix_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            ModeSupport::cpu(
                OperationId::ConvertMatrix,
                mode,
                crate::compute::registry::support::OutputModeRule::ByTargetMode,
                MATRIX_INPUTS,
                &[],
            )
        }
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
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
        | Mode::ImageRgba32F => {
            // NO_PILLOW_SUPPORT: native samples preserve RGB projection for
            // these exact source identities.
            ModeSupport::cpu_core_only(
                OperationId::ConvertMatrix,
                mode,
                crate::compute::registry::support::OutputModeRule::ByTargetMode,
                MATRIX_INPUTS,
                &[],
                "matrix conversion from this exact source identity extends Pillow's RGB matrix contract",
            )
        }
    }
}

/// Matrix conversion support, with one Default strategy for each exact source mode.
pub(crate) static MATRIX_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::ConvertMatrix, &[], classify_matrix_mode)
        .with_rejected_output(OutputModeRule::ByTargetMode);

pub(crate) use super::helpers::convert::prepared::{
    ConvertWrapperMetadata, Prepared, PreparedConvertOutput,
    prepare_pipeline as prepare_convert_pipeline,
};

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_convert_lab(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgbPremultiplied8
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
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Source-mode contract for typed RGB-family-to-LAB pipeline strategies.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::ConvertLab, &[], classify_convert_lab)
        .with_rejected_output(OutputModeRule::Fixed(Mode::ImageLab8));

/// Prepared Convert plan exported through the color-family registry facade.
pub(crate) type PreparedConvert<'a> = Prepared<'a>;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn classify_remap_palette(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLumaA8
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
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "illegal image mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Exact-mode contracts for the native L and P index remapping operation.
pub(crate) static REMAP_PALETTE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::RemapPalette, &[], classify_remap_palette)
        .with_rejected_output(OutputModeRule::Fixed(Mode::ImagePalette8));

pub(crate) const REMAP_PALETTE_DESTINATION_MAP_INPUT: InputContract =
    InputContract::RemapPaletteDestinationMap {
        max_entries: 256,
        rejection_reason: "byte must be in range(0, 256)",
    };

/// Prepared RGB-family-to-LAB execution with exact mode and backend selection.
pub(crate) struct PreparedConvertLab<'a> {
    source: &'a DynamicImage,
    selection: StrategySelection,
}

impl PreparedConvertLab<'_> {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_selected_convert_lab(self.source, self.selection)
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute_selected_convert_lab(
    source: &DynamicImage,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match source {
        DynamicImage::ImageRgb8(source) => super::rgb8::ops::execute(source, selection),
        DynamicImage::ImageRgba8(source) => super::rgba8::ops::execute(source, selection),
        DynamicImage::ImageRgbx8(source) => super::rgbx8::ops::execute(source, selection),
        _ => Err(PilError::InternalError(
            "ConvertLab selection did not match its prepared source mode".into(),
        )),
    }
}

/// Prepares the selected source-mode strategy using the image's variant tag.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    ctx: &ExecutionContext,
) -> Result<PreparedConvertLab<'a>, PilError> {
    let mode = input.mode();
    let assessment = SUPPORT.for_mode(mode).assess(ctx, input.facts())?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "ConvertLab support remained unresolved after source-mode preparation".into(),
            ));
        }
    };
    Ok(PreparedConvertLab {
        source: input.image,
        selection,
    })
}

/// Registry entry for ConvertLab; it preserves typed source and output modes.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::ConvertLab { .. }) {
        return Err(PilError::ValueError("expected ConvertLab op".into()));
    }
    prepare(input, ctx).map(crate::compute::registry::PreparedOperation::from_convert_lab)
}

/// Prepared L/P palette-index remapping with the selected native strategy.
pub(crate) enum PreparedRemapPalette<'a> {
    Luma8(super::luma8::ops::Prepared<'a>),
    Palette8(super::palette8::ops::Prepared<'a>),
}

impl PreparedRemapPalette<'_> {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma8(plan) => plan.selection(),
            Self::Palette8(plan) => plan.selection(),
        }
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::Luma8(plan) => plan.execute(),
            Self::Palette8(plan) => plan.execute(),
        }
    }
}

/// Selects the native L/P mode strategy for a typed remap request.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn prepare_remap_palette<'a>(
    input: OperationInputs<'a>,
    dest_map: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<PreparedRemapPalette<'a>, PilError> {
    REMAP_PALETTE_SUPPORT
        .for_mode(input.mode())
        .validate_remap_palette_destination_map(dest_map.len())?;
    match input.image {
        DynamicImage::ImageLuma8(_) => {
            super::luma8::ops::prepare(input, dest_map, ctx).map(PreparedRemapPalette::Luma8)
        }
        DynamicImage::ImagePalette8(_) => {
            super::palette8::ops::prepare(input, dest_map, ctx).map(PreparedRemapPalette::Palette8)
        }
        _ => {
            let mode = input.mode();
            match REMAP_PALETTE_SUPPORT.for_mode(mode) {
                ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
                ModeSupport::Supported { .. } => Err(PilError::InternalError(
                    "RemapPalette support accepted a non-L/P image".into(),
                )),
            }
        }
    }
}

/// Registry entry for RemapPalette, retaining wrapper palette ownership.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn prepare_remap_palette_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::RemapPalette { dest_map } = op else {
        return Err(PilError::ValueError("expected RemapPalette op".into()));
    };
    prepare_remap_palette(input, dest_map, ctx)
        .map(crate::compute::registry::PreparedOperation::from_remap_palette)
}
