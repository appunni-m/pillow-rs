//! Exact ImageLuma8 EffectSpread and EffectNoise strategies.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::pool_cpu::ops::effects::helpers::spread_vector;
use crate::compute::pool_cpu::ops::effects::helpers::noise_vector;
use crate::compute::pool_cpu::ops::effects::helpers::strategy;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RequestFacts,
    StrategyGuard,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use crate::raster::GrayImage;
use super::default::{effect_noise_impl as noise_impl, effect_spread_impl as spread_impl};
use super::{
    sse2::effect_noise_impl as sse2_noise,
    neon::effect_noise_impl as neon_noise,
    wasm_simd128::effect_noise_impl as wasm_noise,
    sse2::effect_spread_impl as sse2_spread,
    neon::effect_spread_impl as neon_spread,
    wasm_simd128::effect_spread_impl as wasm_spread,
};

/// Exact native buffer type carried by `DynamicImage::ImageLuma8`.
pub(crate) type Input = GrayImage;

static SPREAD_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::EffectSpread,
        Mode::ImageLuma8,
        StrategyGuard::EffectSpreadBytes { channels: 1 },
    );
static NOISE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::EffectNoise,
        Mode::ImageLuma8,
        StrategyGuard::EffectNoiseVector,
    );

pub(crate) enum SpreadImplementation<'a> {
    Default(spread_impl::Kernel<'a>),
    Sse2(spread_vector::Kernel<'a>),
    Neon(spread_vector::Kernel<'a>),
    WasmSimd128(spread_vector::Kernel<'a>),
}
pub(crate) type SpreadStrategy<'a> = Prepared<SpreadImplementation<'a>>;

pub(crate) enum NoiseImplementation<'a> {
    Default(noise_impl::Kernel<'a>),
    Sse2(noise_vector::Kernel),
    Neon(noise_vector::Kernel),
    WasmSimd128(noise_vector::Kernel),
}
pub(crate) type NoiseStrategy<'a> = Prepared<NoiseImplementation<'a>>;

pub(crate) const SPREAD_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::EffectSpread,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    super::super::ops::SPREAD_INPUTS,
    &SPREAD_ARCHITECTURES,
);

pub(crate) const NOISE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::EffectNoise,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    super::super::ops::NOISE_INPUTS,
    &NOISE_ARCHITECTURES,
);

pub(crate) fn prepare_spread<'a>(
    input: &'a Input,
    distance: u32,
    ctx: &ExecutionContext,
) -> Result<SpreadStrategy<'a>, PilError> {
    let facts = strategy::facts(input, RequestFacts::EffectSpread { distance });
    let selection = strategy::select(SPREAD_SUPPORT, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => SpreadImplementation::Default(spread_impl::prepare(input, distance)),
        Architecture::Sse2 => SpreadImplementation::Sse2(sse2_spread::prepare(input.as_raw(), input.width(), input.height(), 1, distance)),
        Architecture::Neon => SpreadImplementation::Neon(neon_spread::prepare(input.as_raw(), input.width(), input.height(), 1, distance)),
        Architecture::WasmSimd128 => SpreadImplementation::WasmSimd128(wasm_spread::prepare(input.as_raw(), input.width(), input.height(), 1, distance)),
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for EffectSpread mode ImageLuma8"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}


fn from_spread_vector(output: spread_vector::Output) -> Result<Input, PilError> {
    Input::from_raw(output.width, output.height, output.bytes)
        .ok_or_else(|| PilError::InternalError("EffectSpread vector output shape mismatch".into()))
}

pub(crate) fn execute_spread(plan: SpreadStrategy<'_>) -> Result<DynamicImage, PilError> {
    match plan.implementation {
        SpreadImplementation::Default(kernel) => spread_impl::execute(kernel).map(DynamicImage::ImageLuma8),
        SpreadImplementation::Sse2(kernel) => sse2_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImageLuma8),
        SpreadImplementation::Neon(kernel) => neon_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImageLuma8),
        SpreadImplementation::WasmSimd128(kernel) => wasm_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImageLuma8),
    }
}

pub(crate) fn prepare_noise<'a>(
    input: &'a Input,
    sigma: f64,
    ctx: &ExecutionContext,
) -> Result<NoiseStrategy<'a>, PilError> {
    let facts = strategy::facts(input, RequestFacts::EffectNoise { sigma_bits: sigma.to_bits() });
    let selection = strategy::select(NOISE_SUPPORT, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => NoiseImplementation::Default(noise_impl::prepare(input, sigma)),
        Architecture::Sse2 => NoiseImplementation::Sse2(sse2_noise::prepare(input.width(), input.height(), sigma)),
        Architecture::Neon => NoiseImplementation::Neon(neon_noise::prepare(input.width(), input.height(), sigma)),
        Architecture::WasmSimd128 => NoiseImplementation::WasmSimd128(wasm_noise::prepare(input.width(), input.height(), sigma)),
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for EffectNoise mode ImageLuma8"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute_noise(plan: NoiseStrategy<'_>) -> Result<DynamicImage, PilError> {
    match plan.implementation {
        NoiseImplementation::Default(kernel) => noise_impl::execute(kernel).map(DynamicImage::ImageLuma8),
        NoiseImplementation::Sse2(kernel) => sse2_noise::execute(kernel).map(DynamicImage::ImageLuma8),
        NoiseImplementation::Neon(kernel) => neon_noise::execute(kernel).map(DynamicImage::ImageLuma8),
        NoiseImplementation::WasmSimd128(kernel) => wasm_noise::execute(kernel).map(DynamicImage::ImageLuma8),
    }
}

static LINEAR_GRADIENT_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::LinearGradient,
        Mode::ImageLuma8,
        StrategyGuard::Always,
    );

pub(crate) const LINEAR_GRADIENT_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::LinearGradient,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    &[],
    &LINEAR_GRADIENT_ARCHITECTURES,
);

pub(crate) enum LinearGradientImplementation {
    Default,
    Sse2,
    Neon,
    WasmSimd128,
}
pub(crate) type LinearGradientStrategy = Prepared<LinearGradientImplementation>;

pub(crate) fn prepare_linear_gradient(
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<LinearGradientStrategy, PilError> {
    let implementation = match selection.implementation.architecture {
        Architecture::Default => LinearGradientImplementation::Default,
        Architecture::Sse2 => LinearGradientImplementation::Sse2,
        Architecture::Neon => LinearGradientImplementation::Neon,
        Architecture::WasmSimd128 => LinearGradientImplementation::WasmSimd128,
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for LinearGradient mode ImageLuma8"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute_linear_gradient(
    plan: LinearGradientStrategy,
) -> Result<crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage, PilError> {
    match plan.implementation {
        LinearGradientImplementation::Default => super::default::linear_gradient_impl::execute(),
        LinearGradientImplementation::Sse2 => super::sse2::linear_gradient_impl::execute(),
        LinearGradientImplementation::Neon => super::neon::linear_gradient_impl::execute(),
        LinearGradientImplementation::WasmSimd128 => {
            super::wasm_simd128::linear_gradient_impl::execute()
        }
    }
}

static RADIAL_GRADIENT_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::RadialGradient,
        Mode::ImageLuma8,
        StrategyGuard::Always,
    );

pub(crate) const RADIAL_GRADIENT_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::RadialGradient,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    &[],
    &RADIAL_GRADIENT_ARCHITECTURES,
);

pub(crate) enum RadialGradientImplementation {
    Default,
    Sse2,
    Neon,
    WasmSimd128,
}
pub(crate) type RadialGradientStrategy = Prepared<RadialGradientImplementation>;

pub(crate) fn prepare_radial_gradient(
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<RadialGradientStrategy, PilError> {
    let implementation = match selection.implementation.architecture {
        Architecture::Default => RadialGradientImplementation::Default,
        Architecture::Sse2 => RadialGradientImplementation::Sse2,
        Architecture::Neon => RadialGradientImplementation::Neon,
        Architecture::WasmSimd128 => RadialGradientImplementation::WasmSimd128,
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for RadialGradient mode ImageLuma8"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute_radial_gradient(
    plan: RadialGradientStrategy,
) -> Result<crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage, PilError> {
    match plan.implementation {
        RadialGradientImplementation::Default => super::default::radial_gradient_impl::execute(),
        RadialGradientImplementation::Sse2 => super::sse2::radial_gradient_impl::execute(),
        RadialGradientImplementation::Neon => super::neon::radial_gradient_impl::execute(),
        RadialGradientImplementation::WasmSimd128 => {
            super::wasm_simd128::radial_gradient_impl::execute()
        }
    }
}

static MANDELBROT_ARCHITECTURES: [ImplementationSupport; 4] = [
    ImplementationSupport::cpu(
        OperationId::EffectMandelbrot,
        Mode::ImageLuma8,
        Architecture::Avx,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx",
            not(target_feature = "avx512f")
        )),
        &[Architecture::Avx],
        StrategyGuard::EffectMandelbrotVector,
    ),
    ImplementationSupport::cpu(
        OperationId::EffectMandelbrot,
        Mode::ImageLuma8,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "avx"),
            not(target_feature = "avx512f")
        )),
        &[Architecture::Sse2],
        StrategyGuard::EffectMandelbrotVector,
    ),
    ImplementationSupport::cpu(
        OperationId::EffectMandelbrot,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::EffectMandelbrotVector,
    ),
    ImplementationSupport::cpu(
        OperationId::EffectMandelbrot,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::EffectMandelbrotVector,
    ),
];

pub(crate) const MANDELBROT_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::EffectMandelbrot,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    super::super::ops::MANDELBROT_INPUTS,
    &MANDELBROT_ARCHITECTURES,
);

pub(crate) enum MandelbrotImplementation {
    Default,
    Avx,
    Sse2,
    Neon,
    WasmSimd128,
}
pub(crate) type MandelbrotStrategy = Prepared<MandelbrotImplementation>;

pub(crate) fn prepare_mandelbrot(
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<MandelbrotStrategy, PilError> {
    let implementation = match selection.implementation.architecture {
        Architecture::Default => MandelbrotImplementation::Default,
        Architecture::Avx => MandelbrotImplementation::Avx,
        Architecture::Sse2 => MandelbrotImplementation::Sse2,
        Architecture::Neon => MandelbrotImplementation::Neon,
        Architecture::WasmSimd128 => MandelbrotImplementation::WasmSimd128,
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for EffectMandelbrot mode ImageLuma8"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute_mandelbrot(
    plan: MandelbrotStrategy,
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage, PilError> {
    match plan.implementation {
        MandelbrotImplementation::Default => {
            super::default::mandelbrot_impl::execute(size, extent, quality)
        }
        MandelbrotImplementation::Avx => {
            super::avx::mandelbrot_impl::execute(size, extent, quality)
        }
        MandelbrotImplementation::Sse2 => {
            super::sse2::mandelbrot_impl::execute(size, extent, quality)
        }
        MandelbrotImplementation::Neon => {
            super::neon::mandelbrot_impl::execute(size, extent, quality)
        }
        MandelbrotImplementation::WasmSimd128 => {
            super::wasm_simd128::mandelbrot_impl::execute(size, extent, quality)
        }
    }
}
