//! Exact-mode support and architecture selection for ImageYCbCr8 Reduce.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    RequestFacts, StrategyInputs,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const INPUTS: &[InputContract] =
    &[InputContract::ReduceFactors { identity_only: false }];

static ARCHITECTURES: [ImplementationSupport; 4] =
    super::super::helpers::strategy::native_byte_architectures(
        OperationId::Reduce,
        Mode::ImageYCbCr8,
        3,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Reduce,
    Mode::ImageYCbCr8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default {
        input: &'a crate::raster::RgbImage,
        x_factor: u32,
        y_factor: u32,
    },
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Avx2 {
        input: &'a crate::raster::RgbImage,
        x_factor: u32,
        y_factor: u32,
    },
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
    Sse2 {
        input: &'a crate::raster::RgbImage,
        x_factor: u32,
        y_factor: u32,
    },
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    Neon {
        input: &'a crate::raster::RgbImage,
        x_factor: u32,
        y_factor: u32,
    },
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    WasmSimd128 {
        input: &'a crate::raster::RgbImage,
        x_factor: u32,
        y_factor: u32,
    },
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn prepare<'a>(
    input: &'a crate::raster::RgbImage,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let (x_factor, y_factor) = match facts.request {
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        _ => return Err(PilError::InternalError(
            "Reduce mode strategy received facts for another operation".into(),
        )),
    };
    let selection = super::super::helpers::strategy::select_mode(SUPPORT, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default { input, x_factor, y_factor },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::Avx2 => ArchitectureStrategy::Avx2 { input, x_factor, y_factor },
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
        Architecture::Sse2 => ArchitectureStrategy::Sse2 { input, x_factor, y_factor },
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => ArchitectureStrategy::Neon { input, x_factor, y_factor },
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128 { input, x_factor, y_factor },
        architecture => return Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for ImageYCbCr8 Reduce"
        ))),
    };
    Ok(Prepared::new(implementation, selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default { input, x_factor, y_factor } =>
            super::default::reduce_impl::execute(input, x_factor, y_factor),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        ArchitectureStrategy::Avx2 { input, x_factor, y_factor } =>
            super::avx2::reduce_impl::execute(input, x_factor, y_factor),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx2")))]
        ArchitectureStrategy::Sse2 { input, x_factor, y_factor } =>
            super::sse2::reduce_impl::execute(input, x_factor, y_factor),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        ArchitectureStrategy::Neon { input, x_factor, y_factor } =>
            super::neon::reduce_impl::execute(input, x_factor, y_factor),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        ArchitectureStrategy::WasmSimd128 { input, x_factor, y_factor } =>
            super::wasm_simd128::reduce_impl::execute(input, x_factor, y_factor),
    }
}
