//! Grayscale's complete source-mode contract and composed executable strategies.
//! The wrapper's typed mode cache resolves successful results to ImageLuma8;
//! eager RGB and single-op lazy P/PA enter this prepared owner with the context.

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

/// Executes the exact mode's Default grayscale kernel when composed by Convert.
///
/// Convert has no vector implementation for its other targets. Its luma
/// conversion therefore composes the same serial Default kernel without
/// manufacturing a second context or making an unreported architecture choice.
pub(crate) fn execute_default(input: GrayscaleInputs<'_>) -> Result<DynamicImage, PilError> {
    macro_rules! fallible {
        ($module:ident, $image:expr) => {{
            let kernel = super::$module::default::grayscale_impl::prepare($image)?;
            super::$module::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }};
    }
    macro_rules! infallible {
        ($module:ident, $image:expr) => {{
            let kernel = super::$module::default::grayscale_impl::prepare($image);
            Ok(DynamicImage::ImageLuma8(
                super::$module::default::grayscale_impl::execute(kernel),
            ))
        }};
    }

    match input.image {
        DynamicImage::One(image) => infallible!(one, image),
        DynamicImage::ImageLuma8(image) => fallible!(luma8, image),
        DynamicImage::ImageLumaA8(image) => fallible!(luma_a8, image),
        DynamicImage::ImageLaPremultiplied8(_) => Err(PilError::ValueError(LA_REJECTION.into())),
        DynamicImage::ImagePalette8(image) => {
            let kernel = super::palette8::default::grayscale_impl::prepare(image, input.palette)?;
            super::palette8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            let kernel = super::palette_alpha8::default::grayscale_impl::prepare(
                image,
                input.palette,
            )?;
            super::palette_alpha8::default::grayscale_impl::execute(kernel)
                .map(DynamicImage::ImageLuma8)
        }
        DynamicImage::ImageRgb8(image) => fallible!(rgb8, image),
        DynamicImage::ImageRgba8(image) => fallible!(rgba8, image),
        DynamicImage::ImageRgbPremultiplied8(image) => fallible!(rgb_premultiplied8, image),
        DynamicImage::ImageRgbx8(image) => fallible!(rgbx8, image),
        DynamicImage::ImageCmyk8(image) => fallible!(cmyk8, image),
        DynamicImage::ImageYCbCr8(image) => infallible!(ycbcr8, image),
        DynamicImage::ImageLab8(image) => fallible!(lab8, image),
        DynamicImage::ImageHsv8(image) => fallible!(hsv8, image),
        DynamicImage::ImageI32(image) => infallible!(i32, image),
        DynamicImage::ImageF32(image) => infallible!(f32, image),
        DynamicImage::ImageI16(image) => fallible!(i16, image),
        DynamicImage::ImageI16L(image) => fallible!(i16l, image),
        DynamicImage::ImageI16B(image) => fallible!(i16b, image),
        DynamicImage::ImageI16N(image) => fallible!(i16n, image),
        DynamicImage::ImageLumaA16(image) => fallible!(luma_a16, image),
        DynamicImage::ImageRgb16(image) => fallible!(rgb16, image),
        DynamicImage::ImageRgba16(image) => fallible!(rgba16, image),
        DynamicImage::ImageRgb32F(image) => fallible!(rgb32f, image),
        DynamicImage::ImageRgba32F(image) => fallible!(rgba32f, image),
    }
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
