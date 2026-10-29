//! ImageOps.expand strategy for exact ImageI16 storage.

use crate::compute::{ExecutionContext, pool_cpu::helpers::execution::prepared::Prepared};
use crate::compute::registry::support::{InputContract, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, StrategyInputs};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

#[path = "default/expand_impl.rs"]
mod default;

const INPUTS: &[InputContract] = &[InputContract::ExpandedDimensions { bytes_per_pixel: 2 }];
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Expand,
    Mode::ImageI16,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::Kernel<'a>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn prepare<'a>(
    input: &'a default::Input,
    border: u32,
    fill: (u8, u8, u8, u8),
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = StrategyInputs {
        width: input.width(),
        height: input.height(),
        byte_len: Some(std::mem::size_of_val(input.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: Default::default(),
    }
    .with_expand_border(border);
    let selection = super::super::helpers::strategy::select_expand(SUPPORT, facts, ctx)?;
    let kernel = default::prepare(input, border, fill);
    Ok(Prepared::new(ArchitectureStrategy::Default(kernel), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(kernel) => default::execute(kernel).map(DynamicImage::ImageI16),
    }
}
