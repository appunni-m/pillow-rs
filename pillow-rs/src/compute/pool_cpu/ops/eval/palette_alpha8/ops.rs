//! ImagePaletteAlpha8 Eval support and Default architecture selection.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, GrayAlphaImage};

use super::default::eval_impl as default;

const INPUTS: &[InputContract] = &[
    InputContract::ByteLutEntries { channels: 2 },
    InputContract::ExactNativeBytes {
        channels: 2,
        overflow_reason: "Eval native byte dimensions overflowed",
        mismatch_reason: "Eval source byte length did not match its dimensions",
    },
];

pub(crate) const SUPPORT: ModeSupport =
    ModeSupport::serial(OperationId::Eval, Mode::ImagePaletteAlpha8, INPUTS);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::Kernel<'a>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn assess(
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    SUPPORT.assess(ctx, input)
}

pub(crate) fn prepare<'a>(
    input: &'a GrayAlphaImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = super::super::helpers::strategy::facts(
        input.width(),
        input.height(),
        input.as_raw().len(),
        lut.len(),
    );
    let selection = super::super::helpers::strategy::select(SUPPORT, facts, ctx)?;
    let kernel = default::prepare(input, lut);
    Ok(Prepared::new(ArchitectureStrategy::Default(kernel), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(kernel) => {
            default::execute(kernel).map(DynamicImage::ImagePaletteAlpha8)
        }
    }
}
