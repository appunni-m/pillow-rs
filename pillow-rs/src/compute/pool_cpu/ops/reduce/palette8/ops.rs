//! Exact-mode support and architecture selection for ImagePalette8 Reduce.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const INPUTS: &[InputContract] = &[InputContract::ReduceFactors {
    identity_only: true,
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Reduce,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default { input: &'a crate::raster::GrayImage },
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn prepare<'a>(
    input: &'a crate::raster::GrayImage,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let selection = super::super::helpers::strategy::select_mode(SUPPORT, facts, ctx)?;
    Ok(Prepared::new(
        ArchitectureStrategy::Default { input },
        selection,
    ))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default { input } => super::default::reduce_impl::execute(input),
    }
}
