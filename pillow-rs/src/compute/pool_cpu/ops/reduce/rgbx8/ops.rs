//! Exact-mode support and architecture selection for ImageRgbx8 Reduce.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, RequestFacts,
    StrategyInputs,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const INPUTS: &[InputContract] =
    &[InputContract::ReduceFactors { identity_only: false }];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(OperationId::Reduce, Mode::ImageRgbx8, OutputModeRule::Preserve, INPUTS, &[]);

pub(crate) enum ArchitectureStrategy<'a> {
    Default {
        input: &'a crate::raster::RgbaImage,
        x_factor: u32,
        y_factor: u32,
    },
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn prepare<'a>(
    input: &'a crate::raster::RgbaImage,
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
    Ok(Prepared::new(
        ArchitectureStrategy::Default { input, x_factor, y_factor },
        selection,
    ))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default { input, x_factor, y_factor } =>
            super::default::reduce_impl::execute(input, x_factor, y_factor),
    }
}
