//! Effect operation contracts and exact-mode strategy composition.
//!
//! TODO(pillow-rs#3ptb): retire the remaining compatibility SIMD registrations
//! after every pipeline and compatibility caller uses the prepared CPU
//! strategies with its injected context. Rayon scheduling remains owned by
//! pool_parallel.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    PaletteKnowledge, RejectionKind, RequestFacts, StrategyInputs, StrategySelection,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;

pub(crate) const SPREAD_INPUTS: &[InputContract] = &[InputContract::EffectSpreadDistance];
pub(crate) const NOISE_INPUTS: &[InputContract] = &[InputContract::EffectNoiseSigma];
pub(crate) const MANDELBROT_INPUTS: &[InputContract] =
    &[InputContract::EffectMandelbrotParameters];
const NO_INPUTS: &[InputContract] = &[];

pub(crate) enum PreparedLinearGradient {
    One(super::one::ops::LinearGradientStrategy),
    ImageLuma8(super::luma8::ops::LinearGradientStrategy),
    ImagePalette8(super::palette8::ops::LinearGradientStrategy),
    ImageI32(super::i32::ops::LinearGradientStrategy),
    ImageF32(super::f32::ops::LinearGradientStrategy),
}

pub(crate) enum PreparedRadialGradient {
    One(super::one::ops::RadialGradientStrategy),
    ImageLuma8(super::luma8::ops::RadialGradientStrategy),
    ImagePalette8(super::palette8::ops::RadialGradientStrategy),
    ImageI32(super::i32::ops::RadialGradientStrategy),
    ImageF32(super::f32::ops::RadialGradientStrategy),
}

pub(crate) struct PreparedMandelbrot {
    strategy: super::luma8::ops::MandelbrotStrategy,
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
}

fn rejected_gradient_mode(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: "image has wrong mode",
        kind: RejectionKind::ValueError,
    }
}

fn classify_linear_gradient(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::LINEAR_GRADIENT_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::LINEAR_GRADIENT_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::LINEAR_GRADIENT_SUPPORT,
        Mode::ImageI32 => super::i32::ops::LINEAR_GRADIENT_SUPPORT,
        Mode::ImageF32 => super::f32::ops::LINEAR_GRADIENT_SUPPORT,
        Mode::ImageLumaA8
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
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => rejected_gradient_mode(mode),
    }
}

fn classify_radial_gradient(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::RADIAL_GRADIENT_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::RADIAL_GRADIENT_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::RADIAL_GRADIENT_SUPPORT,
        Mode::ImageI32 => super::i32::ops::RADIAL_GRADIENT_SUPPORT,
        Mode::ImageF32 => super::f32::ops::RADIAL_GRADIENT_SUPPORT,
        Mode::ImageLumaA8
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
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => rejected_gradient_mode(mode),
    }
}

fn classify_mandelbrot(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::MANDELBROT_SUPPORT,
        Mode::One
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
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
            reason: "Mandelbrot output mode is fixed to L",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Exhaustive support contract for direct eager LinearGradient construction.
pub(crate) static LINEAR_GRADIENT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::LinearGradient,
    NO_INPUTS,
    classify_linear_gradient,
);

/// Exhaustive support contract for direct eager RadialGradient construction.
pub(crate) static RADIAL_GRADIENT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::RadialGradient,
    NO_INPUTS,
    classify_radial_gradient,
);

/// Mandelbrot is a fixed-L constructor; mode classification describes output identity.
pub(crate) static MANDELBROT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::EffectMandelbrot,
    MANDELBROT_INPUTS,
    classify_mandelbrot,
);

impl PreparedMandelbrot {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.strategy.selection()
    }

    pub(crate) fn execute(self) -> Result<GeneratedImage, PilError> {
        super::luma8::ops::execute_mandelbrot(
            self.strategy,
            self.size,
            self.extent,
            self.quality,
        )
    }
}

impl PreparedLinearGradient {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::One(plan) => plan.selection(),
            Self::ImageLuma8(plan) => plan.selection(),
            Self::ImagePalette8(plan) => plan.selection(),
            Self::ImageI32(plan) => plan.selection(),
            Self::ImageF32(plan) => plan.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<GeneratedImage, PilError> {
        match self {
            Self::One(plan) => super::one::ops::execute_linear_gradient(plan),
            Self::ImageLuma8(plan) => super::luma8::ops::execute_linear_gradient(plan),
            Self::ImagePalette8(plan) => super::palette8::ops::execute_linear_gradient(plan),
            Self::ImageI32(plan) => super::i32::ops::execute_linear_gradient(plan),
            Self::ImageF32(plan) => super::f32::ops::execute_linear_gradient(plan),
        }
    }
}

impl PreparedRadialGradient {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::One(plan) => plan.selection(),
            Self::ImageLuma8(plan) => plan.selection(),
            Self::ImagePalette8(plan) => plan.selection(),
            Self::ImageI32(plan) => plan.selection(),
            Self::ImageF32(plan) => plan.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<GeneratedImage, PilError> {
        match self {
            Self::One(plan) => super::one::ops::execute_radial_gradient(plan),
            Self::ImageLuma8(plan) => super::luma8::ops::execute_radial_gradient(plan),
            Self::ImagePalette8(plan) => super::palette8::ops::execute_radial_gradient(plan),
            Self::ImageI32(plan) => super::i32::ops::execute_radial_gradient(plan),
            Self::ImageF32(plan) => super::f32::ops::execute_radial_gradient(plan),
        }
    }
}

pub(crate) fn prepare_linear_gradient(
    mode: Mode,
    ctx: &ExecutionContext,
) -> Result<PreparedLinearGradient, PilError> {
    let selection = LINEAR_GRADIENT_SUPPORT.for_mode(mode).select_serial(ctx)?;
    match mode {
        Mode::One => super::one::ops::prepare_linear_gradient(selection)
            .map(PreparedLinearGradient::One),
        Mode::ImageLuma8 => super::luma8::ops::prepare_linear_gradient(selection)
            .map(PreparedLinearGradient::ImageLuma8),
        Mode::ImagePalette8 => super::palette8::ops::prepare_linear_gradient(selection)
            .map(PreparedLinearGradient::ImagePalette8),
        Mode::ImageI32 => super::i32::ops::prepare_linear_gradient(selection)
            .map(PreparedLinearGradient::ImageI32),
        Mode::ImageF32 => super::f32::ops::prepare_linear_gradient(selection)
            .map(PreparedLinearGradient::ImageF32),
        Mode::ImageLumaA8
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
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::InternalError(
            "rejected LinearGradient mode produced a strategy selection".into(),
        )),
    }
}

pub(crate) fn prepare_radial_gradient(
    mode: Mode,
    ctx: &ExecutionContext,
) -> Result<PreparedRadialGradient, PilError> {
    let selection = RADIAL_GRADIENT_SUPPORT.for_mode(mode).select_serial(ctx)?;
    match mode {
        Mode::One => Ok(PreparedRadialGradient::One(
            super::one::ops::prepare_radial_gradient(selection),
        )),
        Mode::ImageLuma8 => super::luma8::ops::prepare_radial_gradient(selection)
            .map(PreparedRadialGradient::ImageLuma8),
        Mode::ImagePalette8 => Ok(PreparedRadialGradient::ImagePalette8(
            super::palette8::ops::prepare_radial_gradient(selection),
        )),
        Mode::ImageI32 => Ok(PreparedRadialGradient::ImageI32(
            super::i32::ops::prepare_radial_gradient(selection),
        )),
        Mode::ImageF32 => Ok(PreparedRadialGradient::ImageF32(
            super::f32::ops::prepare_radial_gradient(selection),
        )),
        Mode::ImageLumaA8
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
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::InternalError(
            "rejected RadialGradient mode produced a strategy selection".into(),
        )),
    }
}

pub(crate) fn prepare_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
    ctx: &ExecutionContext,
) -> Result<PreparedMandelbrot, PilError> {
    let (x0, y0, x1, y1) = extent;
    let facts = StrategyInputs {
        width: size.0,
        height: size.1,
        byte_len: None,
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::EffectMandelbrot {
            extent_bits: (x0.to_bits(), y0.to_bits(), x1.to_bits(), y1.to_bits()),
            quality,
        },
    };
    let selection = MANDELBROT_SUPPORT
        .for_mode(Mode::ImageLuma8)
        .select(ctx, facts)?;
    let strategy = super::luma8::ops::prepare_mandelbrot(selection)?;
    Ok(PreparedMandelbrot {
        strategy,
        size,
        extent,
        quality,
    })
}

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
