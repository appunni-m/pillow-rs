//! Exact ImagePaletteAlpha8 EffectSpread and EffectNoise strategies.

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
use crate::raster::GrayAlphaImage;
use super::default::{effect_noise_impl as noise_impl, effect_spread_impl as spread_impl};
use super::{
    sse2::effect_noise_impl as sse2_noise,
    neon::effect_noise_impl as neon_noise,
    wasm_simd128::effect_noise_impl as wasm_noise,
    sse2::effect_spread_impl as sse2_spread,
    neon::effect_spread_impl as neon_spread,
    wasm_simd128::effect_spread_impl as wasm_spread,
};

/// Exact native buffer type carried by `DynamicImage::ImagePaletteAlpha8`.
pub(crate) type Input = GrayAlphaImage;

static SPREAD_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::EffectSpread,
        Mode::ImagePaletteAlpha8,
        StrategyGuard::EffectSpreadBytes { channels: 2 },
    );
static NOISE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::vector_architectures_for(
        OperationId::EffectNoise,
        Mode::ImagePaletteAlpha8,
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
    Mode::ImagePaletteAlpha8,
    OutputModeRule::Preserve,
    super::super::ops::SPREAD_INPUTS,
    &SPREAD_ARCHITECTURES,
);

// NO_PILLOW_SUPPORT: Pillow takes a size for EffectNoise. This core route accepts a native
// image only as its dimension carrier.
pub(crate) const NOISE_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::EffectNoise,
    Mode::ImagePaletteAlpha8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    super::super::ops::NOISE_INPUTS,
    &NOISE_ARCHITECTURES,
    "Core-only dimension-carrier mode for EffectNoise.",
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
        Architecture::Sse2 => SpreadImplementation::Sse2(sse2_spread::prepare(input.as_raw(), input.width(), input.height(), 2, distance)),
        Architecture::Neon => SpreadImplementation::Neon(neon_spread::prepare(input.as_raw(), input.width(), input.height(), 2, distance)),
        Architecture::WasmSimd128 => SpreadImplementation::WasmSimd128(wasm_spread::prepare(input.as_raw(), input.width(), input.height(), 2, distance)),
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for EffectSpread mode ImagePaletteAlpha8"
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
        SpreadImplementation::Default(kernel) => spread_impl::execute(kernel).map(DynamicImage::ImagePaletteAlpha8),
        SpreadImplementation::Sse2(kernel) => sse2_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImagePaletteAlpha8),
        SpreadImplementation::Neon(kernel) => neon_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImagePaletteAlpha8),
        SpreadImplementation::WasmSimd128(kernel) => wasm_spread::execute(kernel).and_then(from_spread_vector).map(DynamicImage::ImagePaletteAlpha8),
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
            "unexpected {architecture:?} architecture selected for EffectNoise mode ImagePaletteAlpha8"
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
