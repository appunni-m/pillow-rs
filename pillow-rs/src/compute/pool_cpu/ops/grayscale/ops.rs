//! Grayscale's complete source-mode contract and composed executable strategies.
//!
//! TODO(pillow-rs#3ptb): finish remaining wrapper output-mode transitions and
//! route the serial pool plus any remaining direct callers through this
//! prepared callback with the injected context. Eager RGB and single-op lazy
//! P/PA routes already prepare here.

// NO_PILLOW_SUPPORT: Internal high-depth raster identities have explicit
// typed-input grayscale strategies; Pillow defines no corresponding strings.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport, OperationId, OperationSupport, RejectionKind, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) use crate::compute::registry::support::OperationInputs as GrayscaleInputs;

const LA_REJECTION: &str = "conversion from La to L not supported";

macro_rules! grayscale_result {
    (fallible, $output:expr) => { $output };
    (infallible, $output:expr) => { Ok($output) };
}

macro_rules! declare_grayscale_modes {
    ($palette:ident; $($mode:ident => $module:ident [$($dependency:ident),*] $result:ident),+ $(,)?) => {
        pub(crate) enum PreparedGrayscale<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::SUPPORT,)+
                Mode::ImageLaPremultiplied8 => ModeSupport::Rejected {
                    mode: Mode::ImageLaPremultiplied8, reason: LA_REJECTION,
                    kind: RejectionKind::ValueError,
                },
            }
        }

        pub(crate) fn assess(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::assess(input, ctx),)+
                Mode::ImageLaPremultiplied8 => Ok(SupportAssessment::Rejected {
                    mode: Mode::ImageLaPremultiplied8, reason: LA_REJECTION,
                    kind: RejectionKind::ValueError,
                }),
            }
        }

        pub(crate) fn prepare<'a>(input: GrayscaleInputs<'a>, ctx: &ExecutionContext)
            -> Result<PreparedGrayscale<'a>, PilError> {
            let GrayscaleInputs { image, palette: $palette } = input;
            match image {
                $(DynamicImage::$mode(buffer) =>
                    super::$module::ops::prepare(buffer, $($dependency,)* ctx)
                        .map(PreparedGrayscale::$mode),)+
                DynamicImage::ImageLaPremultiplied8(_) => Err(PilError::ValueError(LA_REJECTION.into())),
            }
        }

        impl PreparedGrayscale<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => grayscale_result!($result, super::$module::ops::execute(strategy)),)+ }
            }
        }
    };
}

declare_grayscale_modes! {
    palette;
    One => one [] infallible,
    ImageLuma8 => luma8 [] fallible,
    ImageLumaA8 => luma_a8 [] fallible,
    ImagePalette8 => palette8 [palette] fallible,
    ImagePaletteAlpha8 => palette_alpha8 [palette] fallible,
    ImageRgb8 => rgb8 [] fallible,
    ImageRgba8 => rgba8 [] fallible,
    ImageRgbPremultiplied8 => rgb_premultiplied8 [] fallible,
    ImageRgbx8 => rgbx8 [] fallible,
    ImageCmyk8 => cmyk8 [] fallible,
    ImageYCbCr8 => ycbcr8 [] fallible,
    ImageLab8 => lab8 [] fallible,
    ImageHsv8 => hsv8 [] fallible,
    ImageI32 => i32 [] infallible,
    ImageF32 => f32 [] infallible,
    ImageI16 => i16 [] fallible,
    ImageI16L => i16l [] fallible,
    ImageI16B => i16b [] fallible,
    ImageI16N => i16n [] fallible,
    ImageLumaA16 => luma_a16 [] fallible,
    ImageRgb16 => rgb16 [] fallible,
    ImageRgba16 => rgba16 [] fallible,
    ImageRgb32F => rgb32f [] fallible,
    ImageRgba32F => rgba32f [] fallible,
}

pub(crate) static SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Grayscale, &[], classify_mode,
);

pub(crate) fn execute_grayscale(input: GrayscaleInputs<'_>, ctx: &ExecutionContext)
    -> Result<DynamicImage, PilError> {
    prepare(input, ctx)?.execute()
}

pub(crate) fn prepare_pipeline<'a>(
    input: GrayscaleInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::Grayscale) {
        return Err(PilError::ValueError("expected Grayscale op".into()));
    }
    prepare(input, ctx).map(crate::compute::registry::PreparedOperation::from_grayscale)
}
