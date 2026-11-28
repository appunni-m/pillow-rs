//! Exact-mode support and architecture selection for ImageI32 Reduce.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule, RequestFacts,
    StrategyInputs,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const INPUTS: &[InputContract] = &[InputContract::ReduceFactors {
    identity_only: false,
}];

static ARCHITECTURES: [ImplementationSupport; 1] =
    super::super::helpers::strategy::native_i32_architectures(OperationId::Reduce, Mode::ImageI32);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Reduce,
    Mode::ImageI32,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default {
        input: &'a crate::raster::RgbaImage,
        x_factor: u32,
        y_factor: u32,
    },
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Avx512F {
        input: &'a crate::raster::RgbaImage,
        x_factor: u32,
        y_factor: u32,
    },
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: &'a crate::raster::RgbaImage,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let (x_factor, y_factor) = match facts.request {
        RequestFacts::Reduce { x_factor, y_factor } => (x_factor, y_factor),
        _ => {
            return Err(PilError::InternalError(
                "Reduce mode strategy received facts for another operation".into(),
            ));
        }
    };
    let selection = super::super::helpers::strategy::select_mode(SUPPORT, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default {
            input,
            x_factor,
            y_factor,
        },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::Avx512F => ArchitectureStrategy::Avx512F {
            input,
            x_factor,
            y_factor,
        },
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for ImageI32 Reduce"
            )));
        }
    };
    Ok(Prepared::new(implementation, selection))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default {
            input,
            x_factor,
            y_factor,
        } => super::default::reduce_impl::execute(input, x_factor, y_factor),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        ArchitectureStrategy::Avx512F {
            input,
            x_factor,
            y_factor,
        } => super::avx512f::reduce_impl::execute(input, x_factor, y_factor),
    }
}
