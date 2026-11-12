//! Composite operation contracts and exact-mode strategy orchestration.

use std::sync::Arc;

use super::helpers::composite_module_support::COMPOSITE_MODULE_INPUTS;
pub(crate) use super::helpers::{
    composite_wrapper_mask_alpha, validate_composite_wrapper_dimensions,
};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, MergeBandModeRule, ModeSupport, OperationId,
    OperationInputs, OperationSupport, OutputModeRule, RejectionKind, SecondaryInputFacts,
    StrategyGuard, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

// TODO(pillow-rs#3ptb): include CompositeModule, Merge, and BlendModule in CPU
// parity for every supported mode/architecture pair, including exact output
// identity, selected/fallback receipts, tails, and empty inputs.

pub(crate) enum PreparedAlphaComposite<'a> {
    ImageLumaA8(super::luma_a8::ops::Strategy<'a>),
    ImageRgba8(super::rgba8::ops::Strategy<'a>),
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
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
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Complete support contract for Pillow's LA and RGBA AlphaComposite modes.
pub(crate) static ALPHA_COMPOSITE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::AlphaComposite, &[], classify_mode);

impl PreparedAlphaComposite<'_> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::ImageLumaA8(strategy) => strategy.selection(),
            Self::ImageRgba8(strategy) => strategy.selection(),
        }
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::ImageLumaA8(strategy) => strategy.execute(),
            Self::ImageRgba8(strategy) => strategy.execute(),
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn assess(
    inputs: OperationInputs<'_>,
    source: crate::compute::registry::support::SecondaryInputFacts,
    destination_offset: (i32, i32),
    source_offset: (i32, i32),
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let facts = inputs
        .facts()
        .with_alpha_composite(source, destination_offset, source_offset);
    classify_mode(inputs.mode()).assess(ctx, facts)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    source: Arc<DynamicImage>,
    destination_offset: (i32, i32),
    source_offset: (i32, i32),
    ctx: &ExecutionContext,
) -> Result<PreparedAlphaComposite<'a>, PilError> {
    let secondary = crate::compute::registry::support::SecondaryInputFacts {
        mode: source.mode(),
        width: source.width(),
        height: source.height(),
        byte_len: Some(source.as_bytes().len()),
    };
    let selection = match assess(input, secondary, destination_offset, source_offset, ctx)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "AlphaComposite preparation received unresolved image facts".into(),
            ));
        }
    };

    match input.image {
        DynamicImage::ImageLumaA8(destination) => Ok(PreparedAlphaComposite::ImageLumaA8(
            super::luma_a8::ops::prepare(destination, source, selection)?,
        )),
        DynamicImage::ImageRgba8(destination) => Ok(PreparedAlphaComposite::ImageRgba8(
            super::rgba8::ops::prepare(destination, source, selection)?,
        )),
        _ => Err(PilError::InternalError(
            "AlphaComposite support and mode routing disagree".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return Err(PilError::ValueError("expected AlphaComposite op".into()));
    };
    if let ModeSupport::Rejected { reason, kind, .. } = classify_mode(input.mode()) {
        return Err(kind.into_error(reason));
    }
    prepare(input, source.materialized_shared()?, *dest, *src, ctx)
        .map(crate::compute::registry::PreparedOperation::from_alpha_composite)
}

/// Prepares the full-frame RGBA fusion using each operation's declared strategy.
pub(crate) fn prepare_mirror_fusion<'a>(
    image: &'a DynamicImage,
    alpha_composite: &PipelineOp,
    mirror: &PipelineOp,
    ctx: &ExecutionContext,
) -> Result<Option<super::rgba8::ops::MirrorFusionStrategy<'a>>, PilError> {
    let PipelineOp::AlphaComposite {
        source,
        dest: (0, 0),
        src: (0, 0),
    } = alpha_composite
    else {
        return Ok(None);
    };
    if !matches!(mirror, PipelineOp::Mirror) {
        return Ok(None);
    }
    let DynamicImage::ImageRgba8(destination) = image else {
        return Ok(None);
    };

    let source = source.materialized_shared()?;
    let DynamicImage::ImageRgba8(_) = source.as_ref() else {
        return Ok(None);
    };
    let prepared = prepare(
        OperationInputs {
            image,
            palette: crate::compute::registry::support::PaletteInput::default(),
        },
        Arc::clone(&source),
        (0, 0),
        (0, 0),
        ctx,
    )?;
    let PreparedAlphaComposite::ImageRgba8(prepared) = prepared else {
        return Ok(None);
    };
    let selection = *prepared.selection();
    super::rgba8::ops::prepare_mirror_fusion(destination, source, selection, ctx)
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MergeLayout {
    Two,
    Three { lab_encoding: bool },
    Four,
}

impl MergeLayout {
    pub(crate) const fn channels(self) -> u8 {
        match self {
            Self::Two => 2,
            Self::Three { .. } => 3,
            Self::Four => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CompositeLayout {
    Byte { channels: u8 },
    U16 { channels: u8 },
    F32 { channels: u8 },
}

pub(super) fn select_support(
    support: ModeSupport,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
    operation: &'static str,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(format!(
            "{operation} preparation received unresolved input facts"
        ))),
    }
}

pub(super) fn secondary_facts(image: &DynamicImage) -> SecondaryInputFacts {
    SecondaryInputFacts {
        mode: image.mode(),
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
    }
}

macro_rules! composite_module_modes {
    ($apply:ident) => {
        $apply! {
            One => one,
            ImageLuma8 => luma8,
            ImageLumaA8 => luma_a8,
            ImageLaPremultiplied8 => la_premultiplied8,
            ImagePalette8 => palette8,
            ImagePaletteAlpha8 => palette_alpha8,
            ImageRgb8 => rgb8,
            ImageRgba8 => rgba8,
            ImageRgbPremultiplied8 => rgb_premultiplied8,
            ImageRgbx8 => rgbx8,
            ImageCmyk8 => cmyk8,
            ImageYCbCr8 => ycbcr8,
            ImageLab8 => lab8,
            ImageHsv8 => hsv8,
            ImageI32 => i32,
            ImageF32 => f32,
            ImageI16 => i16,
            ImageI16L => i16l,
            ImageI16B => i16b,
            ImageI16N => i16n,
            ImageLumaA16 => luma_a16,
            ImageRgb16 => rgb16,
            ImageRgba16 => rgba16,
            ImageRgb32F => rgb32f,
            ImageRgba32F => rgba32f,
        }
    };
}

macro_rules! define_composite_module_routes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        fn composite_support(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::COMPOSITE_MODULE_MODE_SUPPORT,)+
            }
        }

        fn execute_composite_mode(
            mode: Mode,
            source: &DynamicImage,
            destination: &DynamicImage,
            mask: &DynamicImage,
            mask_alpha: bool,
            selection: &StrategySelection,
        ) -> Result<DynamicImage, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::composite(
                    source,
                    destination,
                    mask,
                    mask_alpha,
                    selection,
                ),)+
            }
        }
    };
}

composite_module_modes!(define_composite_module_routes);

/// Complete exact-mode support contract for Image.composite.
pub(crate) static COMPOSITE_MODULE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::CompositeModule,
    COMPOSITE_MODULE_INPUTS,
    composite_support,
);

pub(crate) struct PreparedCompositeModule<'a> {
    source: &'a DynamicImage,
    destination: Arc<DynamicImage>,
    mask: Arc<DynamicImage>,
    mask_alpha: bool,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedCompositeModule<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_composite_mode(
            self.mode,
            self.source,
            &self.destination,
            &self.mask,
            self.mask_alpha,
            &self.selection,
        )
    }
}

fn prepare_composite_module<'a>(
    input: OperationInputs<'a>,
    destination: Arc<DynamicImage>,
    mask: Arc<DynamicImage>,
    mask_alpha: bool,
    ctx: &ExecutionContext,
) -> Result<PreparedCompositeModule<'a>, PilError> {
    let facts = input.facts().with_composite_inputs(
        input.mode(),
        secondary_facts(&destination),
        secondary_facts(&mask),
        mask_alpha,
    );
    let selection = select_support(composite_support(input.mode()), facts, ctx, "Composite")?;
    Ok(PreparedCompositeModule {
        source: input.image,
        destination,
        mask,
        mask_alpha,
        mode: input.mode(),
        selection,
    })
}

pub(crate) fn prepare_composite_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::CompositeModule {
        other,
        mask,
        mask_alpha,
    } = op
    else {
        return Err(PilError::ValueError("expected CompositeModule op".into()));
    };
    // Resolve the mask before the destination to preserve lazy-input error order.
    let mask = mask.materialized_shared()?;
    let destination = other.materialized_shared()?;
    prepare_composite_module(input, destination, mask, *mask_alpha, ctx)
        .map(crate::compute::registry::PreparedOperation::from_composite_module)
}

const BLEND_LUMA: &[Mode] = &[Mode::ImageLuma8];
const BLEND_LUMA_ALPHA: &[Mode] = &[Mode::ImageLumaA8, Mode::ImageLaPremultiplied8];
const BLEND_RGB: &[Mode] = &[
    Mode::ImageRgb8,
    Mode::ImageLab8,
    Mode::ImageHsv8,
    Mode::ImageYCbCr8,
];
const BLEND_RGBA: &[Mode] = &[
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
];
const BLEND_WRONG_MODES: &[Mode] = &[Mode::One, Mode::ImagePalette8, Mode::ImagePaletteAlpha8];

const fn blend_input_contract(modes: &'static [Mode]) -> InputContract {
    InputContract::BlendInputs {
        compatible_modes: modes,
        wrong_mode_modes: BLEND_WRONG_MODES,
        wrong_mode_rejection: "image has wrong mode",
        mismatch_rejection: "images do not match",
    }
}

const BLEND_LUMA_INPUT: &[InputContract] = &[blend_input_contract(BLEND_LUMA)];
const BLEND_LUMA_ALPHA_INPUT: &[InputContract] = &[blend_input_contract(BLEND_LUMA_ALPHA)];
const BLEND_RGB_INPUT: &[InputContract] = &[blend_input_contract(BLEND_RGB)];
const BLEND_RGBA_INPUT: &[InputContract] = &[blend_input_contract(BLEND_RGBA)];

const BLEND_L_VECTOR_MODES: &[Mode] = &[Mode::ImageLuma8];
const BLEND_LA_VECTOR_MODES: &[Mode] = &[Mode::ImageLumaA8, Mode::ImageLaPremultiplied8];
const BLEND_RGB_VECTOR_MODES: &[Mode] = &[Mode::ImageRgb8, Mode::ImageHsv8, Mode::ImageYCbCr8];
const BLEND_RGBA_VECTOR_MODES: &[Mode] = &[
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
];
const BLEND_AVX_FMA: &[Architecture] = &[Architecture::AvxFma];
const BLEND_NEON: &[Architecture] = &[Architecture::Neon];

const fn blend_vector_architectures(
    mode: Mode,
    channels: u8,
    compatible_modes: &'static [Mode],
) -> [ImplementationSupport; 2] {
    [
        ImplementationSupport::cpu(
            OperationId::BlendModule,
            mode,
            Architecture::AvxFma,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            BLEND_AVX_FMA,
            StrategyGuard::BlendNativeVector {
                channels,
                compatible_modes,
            },
        ),
        ImplementationSupport::cpu(
            OperationId::BlendModule,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            BLEND_NEON,
            StrategyGuard::BlendNativeVector {
                channels,
                compatible_modes,
            },
        ),
    ]
}

static BLEND_L_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageLuma8, 1, BLEND_L_VECTOR_MODES);
static BLEND_LA_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageLumaA8, 2, BLEND_LA_VECTOR_MODES);
static BLEND_LA_PREMULTIPLIED_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageLaPremultiplied8, 2, BLEND_LA_VECTOR_MODES);
static BLEND_RGB_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageRgb8, 3, BLEND_RGB_VECTOR_MODES);
static BLEND_HSV_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageHsv8, 3, BLEND_RGB_VECTOR_MODES);
static BLEND_YCBCR_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageYCbCr8, 3, BLEND_RGB_VECTOR_MODES);
static BLEND_RGBA_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageRgba8, 4, BLEND_RGBA_VECTOR_MODES);
static BLEND_CMYK_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageCmyk8, 4, BLEND_RGBA_VECTOR_MODES);
static BLEND_RGB_PREMULTIPLIED_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageRgbPremultiplied8, 4, BLEND_RGBA_VECTOR_MODES);
static BLEND_RGBX_ARCHITECTURES: [ImplementationSupport; 2] =
    blend_vector_architectures(Mode::ImageRgbx8, 4, BLEND_RGBA_VECTOR_MODES);

fn blend_support(mode: Mode) -> ModeSupport {
    let (inputs, architectures) = match mode {
        Mode::ImageLuma8 => (BLEND_LUMA_INPUT, &BLEND_L_ARCHITECTURES[..]),
        Mode::ImageLumaA8 => (BLEND_LUMA_ALPHA_INPUT, &BLEND_LA_ARCHITECTURES[..]),
        Mode::ImageLaPremultiplied8 => (
            BLEND_LUMA_ALPHA_INPUT,
            &BLEND_LA_PREMULTIPLIED_ARCHITECTURES[..],
        ),
        Mode::ImageRgb8 => (BLEND_RGB_INPUT, &BLEND_RGB_ARCHITECTURES[..]),
        Mode::ImageLab8 => (BLEND_RGB_INPUT, &[] as &[ImplementationSupport]),
        Mode::ImageHsv8 => (BLEND_RGB_INPUT, &BLEND_HSV_ARCHITECTURES[..]),
        Mode::ImageYCbCr8 => (BLEND_RGB_INPUT, &BLEND_YCBCR_ARCHITECTURES[..]),
        Mode::ImageRgba8 => (BLEND_RGBA_INPUT, &BLEND_RGBA_ARCHITECTURES[..]),
        Mode::ImageCmyk8 => (BLEND_RGBA_INPUT, &BLEND_CMYK_ARCHITECTURES[..]),
        Mode::ImageRgbPremultiplied8 => {
            (BLEND_RGBA_INPUT, &BLEND_RGB_PREMULTIPLIED_ARCHITECTURES[..])
        }
        Mode::ImageRgbx8 => (BLEND_RGBA_INPUT, &BLEND_RGBX_ARCHITECTURES[..]),
        Mode::One
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
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
            return ModeSupport::Rejected {
                mode,
                reason: "image has wrong mode",
                kind: RejectionKind::ValueError,
            };
        }
    };
    ModeSupport::cpu(
        OperationId::BlendModule,
        mode,
        OutputModeRule::Preserve,
        inputs,
        architectures,
    )
}

/// Complete exact-mode support contract for Pillow Blend.
pub(crate) static BLEND_MODULE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::BlendModule, &[], blend_support);

pub(crate) struct PreparedBlendModule<'a> {
    source: &'a DynamicImage,
    other: Arc<DynamicImage>,
    alpha: f64,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedBlendModule<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_blend_mode(
            self.mode,
            self.source,
            &self.other,
            self.alpha,
            &self.selection,
        )
    }
}

pub(crate) fn prepare_blend_module<'a>(
    input: OperationInputs<'a>,
    other: Arc<DynamicImage>,
    alpha: f64,
    ctx: &ExecutionContext,
) -> Result<PreparedBlendModule<'a>, PilError> {
    let facts = input
        .facts()
        .with_blend_inputs(secondary_facts(&other), alpha);
    let selection = select_support(blend_support(input.mode()), facts, ctx, "Blend")?;
    Ok(PreparedBlendModule {
        source: input.image,
        other,
        alpha,
        mode: input.mode(),
        selection,
    })
}

pub(crate) fn prepare_blend_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::BlendModule { other, alpha } = op else {
        return Err(PilError::ValueError("expected BlendModule op".into()));
    };
    prepare_blend_module(input, other.materialized_shared()?, *alpha, ctx)
        .map(crate::compute::registry::PreparedOperation::from_blend_module)
}

fn execute_blend_mode(
    mode: Mode,
    source: &DynamicImage,
    other: &DynamicImage,
    alpha: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::blend(source, other, alpha, selection),
        Mode::ImageLumaA8 => super::luma_a8::ops::blend(source, other, alpha, selection),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::blend(source, other, alpha, selection)
        }
        Mode::ImageRgb8 => super::rgb8::ops::blend(source, other, alpha, selection),
        Mode::ImageLab8 => super::lab8::ops::blend(source, other, alpha, selection),
        Mode::ImageHsv8 => super::hsv8::ops::blend(source, other, alpha, selection),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgba8 => super::rgba8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::blend(source, other, alpha, selection)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::blend(source, other, alpha, selection),
        Mode::ImageCmyk8 => super::cmyk8::ops::blend(source, other, alpha, selection),
        Mode::One
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
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
        | Mode::ImageRgba32F => Err(PilError::InternalError(
            "unsupported mode reached Blend execution".into(),
        )),
    }
}

const MERGE_SINGLE: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 1,
    mode_rule: MergeBandModeRule::ExactTarget,
    count_rejection: "wrong number of bands",
    first_mode_rejection: "images do not match",
    mode_rejection: "images do not match",
    size_rejection: "size mismatch",
}];
const MERGE_TWO: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 2,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    first_mode_rejection: "image has wrong mode",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];
const MERGE_THREE: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 3,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    first_mode_rejection: "image has wrong mode",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];
const MERGE_FOUR: &[InputContract] = &[InputContract::MergeBandLayout {
    band_count: 4,
    mode_rule: MergeBandModeRule::LumaOrFirstPalette,
    count_rejection: "wrong number of bands",
    first_mode_rejection: "image has wrong mode",
    mode_rejection: "mode mismatch",
    size_rejection: "size mismatch",
}];

// Pillow validates every band after the first as L and leaves band zero to
// ImagingMerge. The typed path accepts byte-backed One/L/P there. Pillow's
// native multi-band merge reads band zero as bytes without validating its
// storage type; preserve safe one-byte outcomes and reject other carrier types
// rather than reproducing invalid reads from I/F/I;16 storage.
// TODO(pillow-rs#3ptb): freeze invalid first-band and high-depth fault outcomes
// in parity, including the safe replacement for Pillow's invalid raw reads.

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

const fn merge_portable_architectures(mode: Mode, channels: u8) -> [ImplementationSupport; 3] {
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

/// Merge's support mode is the requested output identity, not its first band.
fn merge_support(target_mode: Mode) -> ModeSupport {
    let (input, output) = match target_mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => (MERGE_SINGLE, OutputModeRule::ByTargetMode),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            (MERGE_TWO, OutputModeRule::ByTargetMode)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            (MERGE_THREE, OutputModeRule::ByTargetMode)
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8 | Mode::ImageCmyk8 => {
            (MERGE_FOUR, OutputModeRule::ByTargetMode)
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            return ModeSupport::Rejected {
                mode: target_mode,
                reason: "image has wrong mode",
                kind: RejectionKind::ValueError,
            };
        }
    };
    let architectures: &'static [ImplementationSupport] = match target_mode {
        Mode::ImageLumaA8 => &MERGE_LA_ARCHITECTURES,
        Mode::ImageRgb8 => MERGE_RGB_ARCHITECTURES,
        Mode::ImageRgba8 => &MERGE_RGBA_ARCHITECTURES,
        Mode::ImageCmyk8 => &MERGE_CMYK_ARCHITECTURES,
        _ => &[],
    };
    ModeSupport::cpu(
        OperationId::Merge,
        target_mode,
        output,
        input,
        architectures,
    )
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
    Ok(PreparedMerge {
        source: input.image,
        remaining,
        mode: target,
        selection,
    })
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
        Mode::One => super::one::ops::merge(source, remaining, selection),
        Mode::ImageLuma8 => super::luma8::ops::merge(source, remaining, selection),
        Mode::ImageLumaA8 => super::luma_a8::ops::merge(source, remaining, selection),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::merge(source, remaining, selection)
        }
        Mode::ImagePalette8 => super::palette8::ops::merge(source, remaining, selection),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::merge(source, remaining, selection),
        Mode::ImageRgb8 => super::rgb8::ops::merge(source, remaining, selection),
        Mode::ImageRgba8 => super::rgba8::ops::merge(source, remaining, selection),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::merge(source, remaining, selection)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::merge(source, remaining, selection),
        Mode::ImageCmyk8 => super::cmyk8::ops::merge(source, remaining, selection),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::merge(source, remaining, selection),
        Mode::ImageLab8 => super::lab8::ops::merge(source, remaining, selection),
        Mode::ImageHsv8 => super::hsv8::ops::merge(source, remaining, selection),
        Mode::ImageI32 => super::i32::ops::merge(source, remaining, selection),
        Mode::ImageF32 => super::f32::ops::merge(source, remaining, selection),
        Mode::ImageI16 => super::i16::ops::merge(source, remaining, selection),
        Mode::ImageI16L => super::i16l::ops::merge(source, remaining, selection),
        Mode::ImageI16B => super::i16b::ops::merge(source, remaining, selection),
        Mode::ImageI16N => super::i16n::ops::merge(source, remaining, selection),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::InternalError(
            "unsupported mode reached Merge execution".into(),
        )),
    }
}
