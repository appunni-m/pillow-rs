//! BlendModule mode support, preparation, and exact-mode execution.

use std::sync::Arc;

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OperationSupport, OutputModeRule, RejectionKind, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

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
const BLEND_RGB_VECTOR_MODES: &[Mode] = &[
    Mode::ImageRgb8,
    Mode::ImageHsv8,
    Mode::ImageYCbCr8,
];
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
            StrategyGuard::BlendNativeVector { channels, compatible_modes },
        ),
        ImplementationSupport::cpu(
            OperationId::BlendModule,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            BLEND_NEON,
            StrategyGuard::BlendNativeVector { channels, compatible_modes },
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
        Mode::ImageLab8 => (BLEND_RGB_INPUT, &[]),
        Mode::ImageHsv8 => (BLEND_RGB_INPUT, &BLEND_HSV_ARCHITECTURES[..]),
        Mode::ImageYCbCr8 => (BLEND_RGB_INPUT, &BLEND_YCBCR_ARCHITECTURES[..]),
        Mode::ImageRgba8 => (BLEND_RGBA_INPUT, &BLEND_RGBA_ARCHITECTURES[..]),
        Mode::ImageCmyk8 => (BLEND_RGBA_INPUT, &BLEND_CMYK_ARCHITECTURES[..]),
        Mode::ImageRgbPremultiplied8 => (
            BLEND_RGBA_INPUT,
            &BLEND_RGB_PREMULTIPLIED_ARCHITECTURES[..],
        ),
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
    let facts = input.facts().with_blend_inputs(secondary_facts(&other), alpha);
    let selection = select_support(blend_support(input.mode()), facts, ctx, "Blend")?;
    Ok(PreparedBlendModule { source: input.image, other, alpha, mode: input.mode(), selection })
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
        Mode::ImageLuma8 => super::super::luma8::ops::blend(source, other, alpha, selection),
        Mode::ImageLumaA8 => super::super::luma_a8::ops::blend(source, other, alpha, selection),
        Mode::ImageLaPremultiplied8 => super::super::la_premultiplied8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgb8 => super::super::rgb8::ops::blend(source, other, alpha, selection),
        Mode::ImageLab8 => super::super::lab8::ops::blend(source, other, alpha, selection),
        Mode::ImageHsv8 => super::super::hsv8::ops::blend(source, other, alpha, selection),
        Mode::ImageYCbCr8 => super::super::ycbcr8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgba8 => super::super::rgba8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgbPremultiplied8 => super::super::rgb_premultiplied8::ops::blend(source, other, alpha, selection),
        Mode::ImageRgbx8 => super::super::rgbx8::ops::blend(source, other, alpha, selection),
        Mode::ImageCmyk8 => super::super::cmyk8::ops::blend(source, other, alpha, selection),
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

