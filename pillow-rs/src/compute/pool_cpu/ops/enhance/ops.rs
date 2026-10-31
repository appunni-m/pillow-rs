//! Enhancement support contracts and prepared mode dispatch.
//!
//! Exact mode support and prepared strategy composition for enhancement ops.
//!
//! TODO(pillow-rs#3ptb): add Contrast strategies for One,
//! ImageLaPremultiplied8, ImagePalette8, ImagePaletteAlpha8, ImageLab8,
//! ImageI32, ImageF32, ImageI16, ImageI16L, ImageI16B, ImageI16N,
//! ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F.
//! Route modes accepted by `contrast_degenerate` and Blend through those
//! strategies while preserving their conversion and output behavior.
//! ImageRgbPremultiplied8 remains rejected by the public L-to-RGBa base
//! conversion; its exact error is exposed by this contract. The archived
//! Contrast LUT path now has mode/architecture owners
//! for L, LA, RGB, RGBA, RGBX, and CMYK. `Image::enhance_color` also retains
//! conversion/blend routes for modes outside its six native strategies; add
//! exact-mode strategies only after preserving their conversion, error, and
//! output contracts. The enhancer wrappers consult the operation support
//! declarations and preserve an explicitly pinned backend on the final lazy
//! blend. The public
//! `Image::enhance_sharpness` entry queues work for the prepared CPU strategies;
//! compare those owners with archived vector/admission paths. No standalone
//! SIMD registration remains active, and compatibility requests enter through
//! CPU context selection.
//! ImageI32 and ImageF32 Sharpness read and write their exact little-endian
//! scalar samples in the existing four-byte carrier; these native routes are
//! core extensions and are not reported as Pillow parity.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, RejectionKind,
    StrategySelection,
};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

macro_rules! declare_color_saturation_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedColorSaturation<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_color_saturation_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::COLOR_SATURATION_SUPPORT,)+
                Mode::One
                | Mode::ImageLaPremultiplied8
                | Mode::ImagePalette8
                | Mode::ImagePaletteAlpha8
                | Mode::ImageRgbPremultiplied8
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

        fn prepare_color_saturation_mode<'a>(
            image: &'a DynamicImage,
            facts: crate::compute::registry::support::StrategyInputs,
            factor: f64,
            context: &ExecutionContext,
        ) -> Result<PreparedColorSaturation<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(source) => super::$module::ops::prepare_color_saturation(
                    source, facts.with_color_saturation(factor), factor, context,
                ).map(PreparedColorSaturation::$mode),)+
                DynamicImage::One(_)
                | DynamicImage::ImageLaPremultiplied8(_)
                | DynamicImage::ImagePalette8(_)
                | DynamicImage::ImagePaletteAlpha8(_)
                | DynamicImage::ImageRgbPremultiplied8(_)
                | DynamicImage::ImageYCbCr8(_)
                | DynamicImage::ImageLab8(_)
                | DynamicImage::ImageHsv8(_)
                | DynamicImage::ImageI32(_)
                | DynamicImage::ImageF32(_)
                | DynamicImage::ImageI16(_)
                | DynamicImage::ImageI16L(_)
                | DynamicImage::ImageI16B(_)
                | DynamicImage::ImageI16N(_)
                | DynamicImage::ImageLumaA16(_)
                | DynamicImage::ImageRgb16(_)
                | DynamicImage::ImageRgba16(_)
                | DynamicImage::ImageRgb32F(_)
                | DynamicImage::ImageRgba32F(_) => {
                    Err(rejected_color_saturation_error(image.mode()))
                }
            }
        }

        impl PreparedColorSaturation<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self {
                    $(Self::$mode(strategy) => strategy.selection(),)+
                }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => {
                        super::$module::ops::execute_color_saturation(strategy)
                    },)+
                }
            }
        }
    };
}

declare_color_saturation_modes! {
    ImageLuma8 => luma8,
    ImageLumaA8 => luma_a8,
    ImageRgb8 => rgb8,
    ImageRgba8 => rgba8,
    ImageRgbx8 => rgbx8,
    ImageCmyk8 => cmyk8,
}

macro_rules! declare_contrast_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedContrast<'a> {
            $($mode(super::$module::ops::ContrastStrategy<'a>),)+
        }

        fn classify_contrast_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::CONTRAST_SUPPORT,)+
                Mode::One
                | Mode::ImageLaPremultiplied8
                | Mode::ImagePalette8
                | Mode::ImagePaletteAlpha8
                | Mode::ImageLab8
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
                Mode::ImageRgbPremultiplied8 => ModeSupport::Rejected {
                    mode,
                    reason: "conversion from L to RGBa not supported",
                    kind: RejectionKind::ValueError,
                },
            }
        }

        fn prepare_contrast_mode<'a>(
            image: &'a DynamicImage,
            facts: crate::compute::registry::support::StrategyInputs,
            factor: f64,
            context: &ExecutionContext,
        ) -> Result<PreparedContrast<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(source) => super::$module::ops::prepare_contrast(
                    source, facts, factor, context,
                ).map(PreparedContrast::$mode),)+
                DynamicImage::One(_)
                | DynamicImage::ImageLaPremultiplied8(_)
                | DynamicImage::ImagePalette8(_)
                | DynamicImage::ImagePaletteAlpha8(_)
                | DynamicImage::ImageRgbPremultiplied8(_)
                | DynamicImage::ImageLab8(_)
                | DynamicImage::ImageI32(_)
                | DynamicImage::ImageF32(_)
                | DynamicImage::ImageI16(_)
                | DynamicImage::ImageI16L(_)
                | DynamicImage::ImageI16B(_)
                | DynamicImage::ImageI16N(_)
                | DynamicImage::ImageLumaA16(_)
                | DynamicImage::ImageRgb16(_)
                | DynamicImage::ImageRgba16(_)
                | DynamicImage::ImageRgb32F(_)
                | DynamicImage::ImageRgba32F(_) => {
                    Err(rejected_contrast_error(image.mode()))
                }
            }
        }

        impl PreparedContrast<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self {
                    $(Self::$mode(strategy) => strategy.selection(),)+
                }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute_contrast(strategy),)+
                }
            }
        }
    };
}

declare_contrast_modes! {
    ImageLuma8 => luma8,
    ImageLumaA8 => luma_a8,
    ImageRgb8 => rgb8,
    ImageRgba8 => rgba8,
    ImageRgbx8 => rgbx8,
    ImageCmyk8 => cmyk8,
    ImageYCbCr8 => ycbcr8,
    ImageHsv8 => hsv8,
}

macro_rules! declare_brightness_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedBrightness<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::SUPPORT,)+
                Mode::One | Mode::ImagePalette8 | Mode::ImagePaletteAlpha8 => ModeSupport::Rejected {
                    mode,
                    reason: "image has wrong mode",
                    kind: RejectionKind::ValueError,
                },
            }
        }

        fn prepare_mode<'a>(
            image: &'a DynamicImage,
            facts: crate::compute::registry::support::StrategyInputs,
            factor: f64,
            context: &ExecutionContext,
        ) -> Result<PreparedBrightness<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(source) => super::$module::ops::prepare(source, facts, factor, context)
                    .map(PreparedBrightness::$mode),)+
                DynamicImage::One(_)
                | DynamicImage::ImagePalette8(_)
                | DynamicImage::ImagePaletteAlpha8(_) => {
                    Err(rejected_error(image.mode()))
                }
            }
        }

        impl PreparedBrightness<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self {
                    $(Self::$mode(strategy) => strategy.selection(),)+
                }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+
                }
            }
        }
    };
}

declare_brightness_modes! {
    ImageLuma8 => luma8,
    ImageLumaA8 => luma_a8,
    ImageLaPremultiplied8 => la_premultiplied8,
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

const SHARPNESS_INPUTS: &[InputContract] = &[InputContract::SharpnessFactor];

fn classify_sharpness_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        Mode::ImagePalette8 => ModeSupport::Rejected {
            mode,
            reason: "cannot filter palette images",
            kind: RejectionKind::ValueError,
        },
        Mode::ImagePaletteAlpha8 => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        Mode::ImageLuma8 => super::luma8::ops::SHARPNESS_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SHARPNESS_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SHARPNESS_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SHARPNESS_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SHARPNESS_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SHARPNESS_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SHARPNESS_SUPPORT,
        Mode::ImageI32 => super::i32::ops::SHARPNESS_SUPPORT,
        Mode::ImageF32 => super::f32::ops::SHARPNESS_SUPPORT,
        Mode::ImageI16 => super::i16::ops::SHARPNESS_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::SHARPNESS_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::SHARPNESS_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::SHARPNESS_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SHARPNESS_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SHARPNESS_SUPPORT,
    }
}

/// Exhaustive exact-mode Sharpness support and same-mode Default fallback.
pub(crate) static SHARPNESS_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Sharpness, SHARPNESS_INPUTS, classify_sharpness_mode);

/// A concrete Sharpness mode strategy prepared by that mode’s owner.
pub(crate) enum PreparedSharpness<'a> {
    ImageLuma8(super::luma8::ops::SharpnessStrategy<'a>),
    ImageLumaA8(super::luma_a8::ops::SharpnessStrategy<'a>),
    ImageLaPremultiplied8(super::la_premultiplied8::ops::SharpnessStrategy<'a>),
    ImageRgb8(super::rgb8::ops::SharpnessStrategy<'a>),
    ImageRgba8(super::rgba8::ops::SharpnessStrategy<'a>),
    ImageRgbPremultiplied8(super::rgb_premultiplied8::ops::SharpnessStrategy<'a>),
    ImageRgbx8(super::rgbx8::ops::SharpnessStrategy<'a>),
    ImageCmyk8(super::cmyk8::ops::SharpnessStrategy<'a>),
    ImageYCbCr8(super::ycbcr8::ops::SharpnessStrategy<'a>),
    ImageLab8(super::lab8::ops::SharpnessStrategy<'a>),
    ImageHsv8(super::hsv8::ops::SharpnessStrategy<'a>),
    ImageI32(super::i32::ops::SharpnessStrategy<'a>),
    ImageF32(super::f32::ops::SharpnessStrategy<'a>),
    ImageI16(super::i16::ops::SharpnessStrategy<'a>),
    ImageI16L(super::i16l::ops::SharpnessStrategy<'a>),
    ImageI16B(super::i16b::ops::SharpnessStrategy<'a>),
    ImageI16N(super::i16n::ops::SharpnessStrategy<'a>),
    ImageLumaA16(super::luma_a16::ops::SharpnessStrategy<'a>),
    ImageRgb16(super::rgb16::ops::SharpnessStrategy<'a>),
    ImageRgba16(super::rgba16::ops::SharpnessStrategy<'a>),
    ImageRgb32F(super::rgb32f::ops::SharpnessStrategy<'a>),
    ImageRgba32F(super::rgba32f::ops::SharpnessStrategy<'a>),
}

impl PreparedSharpness<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::ImageLuma8(strategy) => strategy.selection(),
            Self::ImageLumaA8(strategy) => strategy.selection(),
            Self::ImageLaPremultiplied8(strategy) => strategy.selection(),
            Self::ImageRgb8(strategy) => strategy.selection(),
            Self::ImageRgba8(strategy) => strategy.selection(),
            Self::ImageRgbPremultiplied8(strategy) => strategy.selection(),
            Self::ImageRgbx8(strategy) => strategy.selection(),
            Self::ImageCmyk8(strategy) => strategy.selection(),
            Self::ImageYCbCr8(strategy) => strategy.selection(),
            Self::ImageLab8(strategy) => strategy.selection(),
            Self::ImageHsv8(strategy) => strategy.selection(),
            Self::ImageI32(strategy) => strategy.selection(),
            Self::ImageF32(strategy) => strategy.selection(),
            Self::ImageI16(strategy) => strategy.selection(),
            Self::ImageI16L(strategy) => strategy.selection(),
            Self::ImageI16B(strategy) => strategy.selection(),
            Self::ImageI16N(strategy) => strategy.selection(),
            Self::ImageLumaA16(strategy) => strategy.selection(),
            Self::ImageRgb16(strategy) => strategy.selection(),
            Self::ImageRgba16(strategy) => strategy.selection(),
            Self::ImageRgb32F(strategy) => strategy.selection(),
            Self::ImageRgba32F(strategy) => strategy.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::ImageLuma8(strategy) => super::luma8::ops::execute_sharpness(strategy),
            Self::ImageLumaA8(strategy) => super::luma_a8::ops::execute_sharpness(strategy),
            Self::ImageLaPremultiplied8(strategy) => super::la_premultiplied8::ops::execute_sharpness(strategy),
            Self::ImageRgb8(strategy) => super::rgb8::ops::execute_sharpness(strategy),
            Self::ImageRgba8(strategy) => super::rgba8::ops::execute_sharpness(strategy),
            Self::ImageRgbPremultiplied8(strategy) => super::rgb_premultiplied8::ops::execute_sharpness(strategy),
            Self::ImageRgbx8(strategy) => super::rgbx8::ops::execute_sharpness(strategy),
            Self::ImageCmyk8(strategy) => super::cmyk8::ops::execute_sharpness(strategy),
            Self::ImageYCbCr8(strategy) => super::ycbcr8::ops::execute_sharpness(strategy),
            Self::ImageLab8(strategy) => super::lab8::ops::execute_sharpness(strategy),
            Self::ImageHsv8(strategy) => super::hsv8::ops::execute_sharpness(strategy),
            Self::ImageI32(strategy) => super::i32::ops::execute_sharpness(strategy),
            Self::ImageF32(strategy) => super::f32::ops::execute_sharpness(strategy),
            Self::ImageI16(strategy) => super::i16::ops::execute_sharpness(strategy),
            Self::ImageI16L(strategy) => super::i16l::ops::execute_sharpness(strategy),
            Self::ImageI16B(strategy) => super::i16b::ops::execute_sharpness(strategy),
            Self::ImageI16N(strategy) => super::i16n::ops::execute_sharpness(strategy),
            Self::ImageLumaA16(strategy) => super::luma_a16::ops::execute_sharpness(strategy),
            Self::ImageRgb16(strategy) => super::rgb16::ops::execute_sharpness(strategy),
            Self::ImageRgba16(strategy) => super::rgba16::ops::execute_sharpness(strategy),
            Self::ImageRgb32F(strategy) => super::rgb32f::ops::execute_sharpness(strategy),
            Self::ImageRgba32F(strategy) => super::rgba32f::ops::execute_sharpness(strategy),
        }
    }
}

fn prepare_sharpness(
    input: OperationInputs<'_>,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedSharpness<'_>, PilError> {
    prepare_sharpness_mode(input.image, input.facts(), factor, context)
}

fn prepare_sharpness_mode(
    image: &DynamicImage,
    facts: crate::compute::registry::support::StrategyInputs,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedSharpness<'_>, PilError> {
    match image {
        DynamicImage::ImageLuma8(source) => super::luma8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageLuma8),
        DynamicImage::ImageLumaA8(source) => super::luma_a8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageLumaA8),
        DynamicImage::ImageLaPremultiplied8(source) => super::la_premultiplied8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageLaPremultiplied8),
        DynamicImage::ImageRgb8(source) => super::rgb8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgb8),
        DynamicImage::ImageRgba8(source) => super::rgba8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgba8),
        DynamicImage::ImageRgbPremultiplied8(source) => super::rgb_premultiplied8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgbPremultiplied8),
        DynamicImage::ImageRgbx8(source) => super::rgbx8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgbx8),
        DynamicImage::ImageCmyk8(source) => super::cmyk8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageCmyk8),
        DynamicImage::ImageYCbCr8(source) => super::ycbcr8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageYCbCr8),
        DynamicImage::ImageLab8(source) => super::lab8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageLab8),
        DynamicImage::ImageHsv8(source) => super::hsv8::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageHsv8),
        DynamicImage::ImageI32(source) => super::i32::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageI32),
        DynamicImage::ImageF32(source) => super::f32::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageF32),
        DynamicImage::ImageI16(source) => super::i16::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageI16),
        DynamicImage::ImageI16L(source) => super::i16l::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageI16L),
        DynamicImage::ImageI16B(source) => super::i16b::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageI16B),
        DynamicImage::ImageI16N(source) => super::i16n::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageI16N),
        DynamicImage::ImageLumaA16(source) => super::luma_a16::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageLumaA16),
        DynamicImage::ImageRgb16(source) => super::rgb16::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgb16),
        DynamicImage::ImageRgba16(source) => super::rgba16::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgba16),
        DynamicImage::ImageRgb32F(source) => super::rgb32f::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgb32F),
        DynamicImage::ImageRgba32F(source) => super::rgba32f::ops::prepare_sharpness(source, facts, factor, context).map(PreparedSharpness::ImageRgba32F),
        DynamicImage::One(_)
        | DynamicImage::ImagePalette8(_)
        | DynamicImage::ImagePaletteAlpha8(_) => {
            Err(rejected_sharpness_error(image.mode()))
        }
    }
}

fn rejected_sharpness_error(mode: Mode) -> PilError {
    match SHARPNESS_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => PilError::InternalError(
            "Sharpness preparation rejected a mode declared as supported".into(),
        ),
    }
}

ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Sharpness { factor } = operation else {
        return Err(PilError::ValueError("expected Sharpness op".into()));
    };
    prepare_sharpness(input, *factor, context)
        .map(crate::compute::registry::PreparedOperation::from_sharpness)
}

const INPUTS: &[InputContract] = &[];

/// Native prepared-path support for Contrast's currently queued byte modes.
///
/// The public eager conversion modes remain on their existing wrapper routes
/// until their exact mode strategies are migrated.
pub(crate) static CONTRAST_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Contrast, INPUTS, classify_contrast_mode);

/// Exhaustive source-mode support for native Brightness.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Brightness, INPUTS, classify_mode);

/// Native prepared-path support for ColorSaturation's queued modes.
///
/// The public image wrapper keeps its degenerate conversion path eager for
/// other modes; that direct route remains in the migration inventory.
pub(crate) static COLOR_SATURATION_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ColorSaturation,
    INPUTS,
    classify_color_saturation_mode,
);

fn rejected_error(mode: Mode) -> PilError {
    match SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => PilError::InternalError(
            "Brightness dispatch rejected a mode declared as supported".into(),
        ),
    }
}

fn rejected_color_saturation_error(mode: Mode) -> PilError {
    match COLOR_SATURATION_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => PilError::InternalError(
            "ColorSaturation dispatch rejected a mode declared as supported".into(),
        ),
    }
}

fn rejected_contrast_error(mode: Mode) -> PilError {
    match CONTRAST_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => PilError::InternalError(
            "Contrast dispatch rejected a mode declared as supported".into(),
        ),
    }
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedBrightness<'a>, PilError> {
    prepare_mode(input.image, input.facts().with_brightness(factor), factor, context)
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    operation: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Brightness { factor } = operation else {
        return Err(PilError::ValueError("expected Brightness op".into()));
    };
    prepare(input, *factor, context)
        .map(crate::compute::registry::PreparedOperation::from_brightness)
}

pub(crate) fn prepare_color_saturation<'a>(
    input: OperationInputs<'a>,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedColorSaturation<'a>, PilError> {
    prepare_color_saturation_mode(input.image, input.facts(), factor, context)
}

pub(crate) fn prepare_color_saturation_pipeline<'a>(
    input: OperationInputs<'a>,
    operation: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::ColorSaturation { factor } = operation else {
        return Err(PilError::ValueError("expected ColorSaturation op".into()));
    };
    prepare_color_saturation(input, *factor, context)
        .map(crate::compute::registry::PreparedOperation::from_color_saturation)
}

pub(crate) fn prepare_contrast<'a>(
    input: OperationInputs<'a>,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedContrast<'a>, PilError> {
    prepare_contrast_mode(
        input.image,
        input.facts().with_contrast(factor),
        factor,
        context,
    )
}

pub(crate) fn prepare_contrast_pipeline<'a>(
    input: OperationInputs<'a>,
    operation: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Contrast { factor } = operation else {
        return Err(PilError::ValueError("expected Contrast op".into()));
    };
    prepare_contrast(input, *factor, context)
        .map(crate::compute::registry::PreparedOperation::from_contrast)
}
