//! Merge mode support, preparation, and exact-mode execution.

use std::sync::Arc;

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, MergeBandModeRule, ModeSupport, OperationId,
    OperationInputs, OperationSupport, OutputModeRule, RejectionKind, StrategyGuard,
    StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::MergeLayout;

const MERGE_SINGLE: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 1,
    mode_rule: MergeBandModeRule::ExactTarget,
    count_rejection: "wrong number of bands",
    mode_rejection: "images do not match",
    size_rejection: "size mismatch",
}];
const MERGE_TWO: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 2,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];
const MERGE_THREE: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 3,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];
const MERGE_FOUR: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 4,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];

const MERGE_RGB_SSSE3: &[Architecture] = &[Architecture::Ssse3];
const MERGE_RGB_NEON: &[Architecture] = &[Architecture::Neon];
const MERGE_SSE2: &[Architecture] = &[Architecture::Sse2];
const MERGE_NEON: &[Architecture] = &[Architecture::Neon];
const MERGE_WASM_SIMD128: &[Architecture] = &[Architecture::WasmSimd128];

const fn merge_native_architecture(
    mode: Mode,
    channels: u8,
    architecture: Architecture,
    compiled: bool,
    requires: &'static [Architecture],
) -> ImplementationSupport {
    ImplementationSupport::cpu(
        OperationId::Merge,
        mode,
        architecture,
        compiled,
        requires,
        StrategyGuard::MergeNativeInterleave {
            target_mode: mode,
            channels,
        },
    )
}

const fn merge_portable_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        merge_native_architecture(
            mode,
            channels,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            MERGE_SSE2,
        ),
        merge_native_architecture(
            mode,
            channels,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            MERGE_NEON,
        ),
        merge_native_architecture(
            mode,
            channels,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            MERGE_WASM_SIMD128,
        ),
    ]
}

static MERGE_LA_ARCHITECTURES: [ImplementationSupport; 3] =
    merge_portable_architectures(Mode::ImageLumaA8, 2);
static MERGE_RGBA_ARCHITECTURES: [ImplementationSupport; 3] =
    merge_portable_architectures(Mode::ImageRgba8, 4);
static MERGE_CMYK_ARCHITECTURES: [ImplementationSupport; 3] =
    merge_portable_architectures(Mode::ImageCmyk8, 4);

const MERGE_RGB_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Merge,
        Mode::ImageRgb8,
        Architecture::Ssse3,
        cfg!(target_arch = "x86_64"),
        MERGE_RGB_SSSE3,
        StrategyGuard::MergeRgbInterleave,
    ),
    merge_native_architecture(
        Mode::ImageRgb8,
        3,
        Architecture::Ssse3,
        cfg!(target_arch = "x86_64"),
        MERGE_RGB_SSSE3,
    ),
    ImplementationSupport::cpu(
        OperationId::Merge,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        MERGE_RGB_NEON,
        StrategyGuard::MergeRgbInterleave,
    ),
    merge_native_architecture(
        Mode::ImageRgb8,
        3,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        MERGE_RGB_NEON,
    ),
    merge_native_architecture(
        Mode::ImageRgb8,
        3,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        )),
        MERGE_SSE2,
    ),
    merge_native_architecture(
        Mode::ImageRgb8,
        3,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        MERGE_WASM_SIMD128,
    ),
];

pub(crate) struct PreparedMerge<'a> {
    source: &'a DynamicImage,
    remaining: Vec<Arc<DynamicImage>>,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedMerge<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_merge_mode(self.mode, self.source, &self.remaining, &self.selection)
    }
}

fn merge_support(mode: Mode) -> ModeSupport {
    let (input, output) = match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => (MERGE_SINGLE, OutputModeRule::Fixed(mode)),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            (MERGE_TWO, OutputModeRule::Fixed(mode))
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            (MERGE_THREE, OutputModeRule::Fixed(mode))
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => (MERGE_FOUR, OutputModeRule::Fixed(mode)),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            return ModeSupport::Rejected {
                mode,
                reason: "image has wrong mode",
                kind: RejectionKind::ValueError,
            };
        }
    };
    let architectures: &'static [ImplementationSupport] = match mode {
        Mode::ImageLumaA8 => &MERGE_LA_ARCHITECTURES,
        Mode::ImageRgb8 => MERGE_RGB_ARCHITECTURES,
        Mode::ImageRgba8 => &MERGE_RGBA_ARCHITECTURES,
        Mode::ImageCmyk8 => &MERGE_CMYK_ARCHITECTURES,
        _ => &[],
    };
    ModeSupport::cpu(OperationId::Merge, mode, output, input, architectures)
}

/// Complete exact-mode support contract for Image.merge.
pub(crate) static MERGE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Merge, &[], merge_support)
        .with_rejected_output(OutputModeRule::ByTargetMode);

fn merge_facts(
    input: OperationInputs<'_>,
    target: Mode,
    bands: &[Image],
) -> Result<StrategyInputs, PilError> {
    let first_mode = bands
        .first()
        .map(Image::mode_identity)
        .transpose()?
        .unwrap_or_else(|| input.mode());
    if !bands.is_empty() && first_mode != input.mode() {
        return Err(PilError::InternalError(
            "Merge source and first band identities disagree".into(),
        ));
    }
    let first_size = bands
        .first()
        .map(Image::size)
        .transpose()?
        .unwrap_or_else(|| (input.image.width(), input.image.height()));
    let remaining_modes_are_luma = bands
        .iter()
        .skip(1)
        .map(Image::mode_identity)
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .all(|mode| *mode == Mode::ImageLuma8);
    let dimensions_match = (input.image.width(), input.image.height()) == first_size
        && bands
            .iter()
            .skip(1)
            .map(Image::size)
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .all(|size| *size == first_size);
    Ok(input.facts().with_merge_bands(
        target,
        bands.len(),
        first_mode,
        remaining_modes_are_luma,
        dimensions_match,
    ))
}

fn select_merge(
    target: Mode,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match merge_support(target).assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Merge preparation received unresolved band facts".into(),
        )),
    }
}

pub(crate) fn prepare_merge<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    bands: &'a [Image],
    ctx: &ExecutionContext,
) -> Result<PreparedMerge<'a>, PilError> {
    let selection = select_merge(target, merge_facts(input, target, bands)?, ctx)?;
    let remaining = bands
        .iter()
        .skip(1)
        .map(Image::materialized_shared)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedMerge { source: input.image, remaining, mode: target, selection })
}

pub(crate) fn prepare_merge_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Merge { mode, bands } = op else {
        return Err(PilError::ValueError("expected Merge op".into()));
    };
    prepare_merge(input, *mode, bands, ctx)
        .map(crate::compute::registry::PreparedOperation::from_merge)
}

fn execute_merge_mode(
    mode: Mode,
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One => super::super::one::ops::merge(source, remaining, selection),
        Mode::ImageLuma8 => super::super::luma8::ops::merge(source, remaining, selection),
        Mode::ImageLumaA8 => super::super::luma_a8::ops::merge(source, remaining, selection),
        Mode::ImageLaPremultiplied8 => super::super::la_premultiplied8::ops::merge(source, remaining, selection),
        Mode::ImagePalette8 => super::super::palette8::ops::merge(source, remaining, selection),
        Mode::ImagePaletteAlpha8 => super::super::palette_alpha8::ops::merge(source, remaining, selection),
        Mode::ImageRgb8 => super::super::rgb8::ops::merge(source, remaining, selection),
        Mode::ImageRgba8 => super::super::rgba8::ops::merge(source, remaining, selection),
        Mode::ImageRgbPremultiplied8 => super::super::rgb_premultiplied8::ops::merge(source, remaining, selection),
        Mode::ImageRgbx8 => super::super::rgbx8::ops::merge(source, remaining, selection),
        Mode::ImageCmyk8 => super::super::cmyk8::ops::merge(source, remaining, selection),
        Mode::ImageYCbCr8 => super::super::ycbcr8::ops::merge(source, remaining, selection),
        Mode::ImageLab8 => super::super::lab8::ops::merge(source, remaining, selection),
        Mode::ImageHsv8 => super::super::hsv8::ops::merge(source, remaining, selection),
        Mode::ImageI32 => super::super::i32::ops::merge(source, remaining, selection),
        Mode::ImageF32 => super::super::f32::ops::merge(source, remaining, selection),
        Mode::ImageI16 => super::super::i16::ops::merge(source, remaining, selection),
        Mode::ImageI16L => super::super::i16l::ops::merge(source, remaining, selection),
        Mode::ImageI16B => super::super::i16b::ops::merge(source, remaining, selection),
        Mode::ImageI16N => super::super::i16n::ops::merge(source, remaining, selection),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::InternalError(
            "unsupported mode reached Merge execution".into(),
        )),
    }
}

