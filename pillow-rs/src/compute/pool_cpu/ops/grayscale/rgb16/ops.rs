//! ImageRgb16 semantics composed with its serial architecture strategy.

// NO_PILLOW_SUPPORT: typed native RGB16 grayscale extends Pillow.

use crate::compute::{ExecutionContext, registry::support::{InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs, PaletteKnowledge, SupportAssessment}};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::default::grayscale_impl;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Grayscale, Mode::ImageRgb16,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    &[InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true }], &[],
    "typed native RGB16 grayscale extends Pillow for this internal raster identity",
);

pub(crate) enum ArchitectureStrategy<'a> { Default(grayscale_impl::Kernel<'a>) }
pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn assess(input: StrategyInputs, ctx: &ExecutionContext) -> Result<SupportAssessment, PilError> {
    SUPPORT.assess(ctx, input)
}

pub(crate) fn prepare<'a>(input: &'a grayscale_impl::Input, ctx: &ExecutionContext) -> Result<Strategy<'a>, PilError> {
    let implementation = grayscale_impl::prepare(input)?;
    let facts = StrategyInputs { width: input.width(), height: input.height(), byte_len: Some(core::mem::size_of_val(input.as_raw().as_slice())), palette: PaletteKnowledge::Unknown, request: Default::default() };
    let selection = SUPPORT.select(ctx, facts)?;
    Ok(Prepared::new(ArchitectureStrategy::Default(implementation), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
            ArchitectureStrategy::Default(input) => grayscale_impl::execute(input).map(DynamicImage::ImageLuma8),
    }
}
