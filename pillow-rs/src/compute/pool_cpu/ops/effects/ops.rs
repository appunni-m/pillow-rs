//! Effect operation contracts and exact-mode strategy composition.
//!
//! TODO(pillow-rs#3ptb): the CPU pool/executor must route pipeline, eager, and
//! compatibility calls through these prepared strategies with its injected
//! context. Retire the former SIMD registrations after that routing replaces
//! their callers. Rayon scheduling remains owned by pool_parallel.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    StrategySelection,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SPREAD_INPUTS: &[InputContract] = &[InputContract::EffectSpreadDistance];
pub(crate) const NOISE_INPUTS: &[InputContract] = &[InputContract::EffectNoiseSigma];

macro_rules! declare_effect_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedEffectSpread<'a> {
            $($mode(super::$module::ops::SpreadStrategy<'a>),)+
        }

        pub(crate) enum PreparedEffectNoise<'a> {
            $($mode(super::$module::ops::NoiseStrategy<'a>),)+
        }

        fn classify_spread(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::SPREAD_SUPPORT,)+ }
        }

        fn classify_noise(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::NOISE_SUPPORT,)+ }
        }

        fn prepare_spread_mode<'a>(
            image: &'a DynamicImage,
            distance: u32,
            ctx: &ExecutionContext,
        ) -> Result<PreparedEffectSpread<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare_spread(input, distance, ctx)
                    .map(PreparedEffectSpread::$mode),)+
            }
        }

        fn prepare_noise_mode<'a>(
            image: &'a DynamicImage,
            sigma: f64,
            ctx: &ExecutionContext,
        ) -> Result<PreparedEffectNoise<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare_noise(input, sigma, ctx)
                    .map(PreparedEffectNoise::$mode),)+
            }
        }

        impl PreparedEffectSpread<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(plan) => plan.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(plan) => super::$module::ops::execute_spread(plan),)+ }
            }
        }

        impl PreparedEffectNoise<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(plan) => plan.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(plan) => super::$module::ops::execute_noise(plan),)+ }
            }
        }
    };
}

declare_effect_modes! {
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

/// Exhaustive support contract for native-sample pixel spread.
pub(crate) static EFFECT_SPREAD_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::EffectSpread,
    SPREAD_INPUTS,
    classify_spread,
);

/// Exhaustive support contract for L-mode Gaussian noise generation.
pub(crate) static EFFECT_NOISE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::EffectNoise,
    NOISE_INPUTS,
    classify_noise,
);

pub(crate) fn prepare_spread<'a>(
    image: &'a DynamicImage,
    distance: u32,
    ctx: &ExecutionContext,
) -> Result<PreparedEffectSpread<'a>, PilError> {
    prepare_spread_mode(image, distance, ctx)
}

pub(crate) fn prepare_noise<'a>(
    image: &'a DynamicImage,
    sigma: f64,
    ctx: &ExecutionContext,
) -> Result<PreparedEffectNoise<'a>, PilError> {
    prepare_noise_mode(image, sigma, ctx)
}

pub(crate) fn prepare_spread_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::EffectSpread { distance } = op else {
        return Err(PilError::ValueError("expected EffectSpread op".into()));
    };
    prepare_spread(input.image, *distance, ctx)
        .map(crate::compute::registry::PreparedOperation::from_effect_spread)
}

pub(crate) fn prepare_noise_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::EffectNoise { sigma } = op else {
        return Err(PilError::ValueError("expected EffectNoise op".into()));
    };
    prepare_noise(input.image, *sigma, ctx)
        .map(crate::compute::registry::PreparedOperation::from_effect_noise)
}
