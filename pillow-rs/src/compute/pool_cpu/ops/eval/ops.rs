//! Eval operation contract and exact-mode prepared strategy composition.
//!
//! TODO(pillow-rs#3ptb): retire the separate SIMD Eval registration after all
//! eager, fusion, and compatibility callers use these prepared CPU strategies.
//! TODO(pillow-rs#3ptb): move the archived Rayon row path to the later
//! `pool_parallel` owner; do not move its scheduling into this CPU family.
//! TODO(pillow-rs#3ptb): move direct scalar point transforms from
//! ops/module_fns/point.rs into a typed native-sample owner; those calls are
//! separate from this byte-LUT PipelineOp and must retain I/F/I;16 semantics.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    RejectionKind, StrategyInputs, SupportAssessment, StrategySelection,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

macro_rules! declare_eval_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedEval<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::SUPPORT,)+
                Mode::ImageI32 | Mode::ImageF32 | Mode::ImageI16 | Mode::ImageI16L
                | Mode::ImageI16B | Mode::ImageI16N | Mode::ImageLumaA16 | Mode::ImageRgb16
                | Mode::ImageRgba16 | Mode::ImageRgb32F | Mode::ImageRgba32F => rejected(mode),
            }
        }

        pub(crate) fn assess(
            mode: Mode,
            input: StrategyInputs,
            ctx: &ExecutionContext,
        ) -> Result<SupportAssessment, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::assess(input, ctx),)+
                _ => classify_mode(mode).assess(ctx, input),
            }
        }

        fn prepare_image<'a>(
            image: &'a DynamicImage,
            lut: &'a [u8],
            ctx: &ExecutionContext,
        ) -> Result<PreparedEval<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, lut, ctx)
                    .map(PreparedEval::$mode),)+
                DynamicImage::ImageI32(_) | DynamicImage::ImageF32(_)
                | DynamicImage::ImageI16(_) | DynamicImage::ImageI16L(_)
                | DynamicImage::ImageI16B(_) | DynamicImage::ImageI16N(_)
                | DynamicImage::ImageLumaA16(_) | DynamicImage::ImageRgb16(_)
                | DynamicImage::ImageRgba16(_) | DynamicImage::ImageRgb32F(_)
                | DynamicImage::ImageRgba32F(_) => Err(eval_mode_error()),
            }
        }

        impl PreparedEval<'_> {
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

declare_eval_modes! {
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
}

const MODE_ERROR: &str = "point operation not supported for this mode";

fn rejected(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: MODE_ERROR,
        kind: RejectionKind::ValueError,
    }
}

/// Complete support contract for the mode-preserving byte-LUT operation.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Eval, &[], classify_mode);

fn eval_mode_error() -> PilError {
    RejectionKind::ValueError.into_error(MODE_ERROR)
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<PreparedEval<'a>, PilError> {
    prepare_image(image, lut, ctx)
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Eval { lut } = op else {
        return Err(PilError::ValueError("expected Eval op".into()));
    };
    let strategy = prepare(input.image, lut, ctx)?;
    Ok(crate::compute::registry::PreparedOperation::from_eval(strategy))
}
