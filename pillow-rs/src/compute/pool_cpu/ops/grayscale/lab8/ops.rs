//! ImageLab8 semantics composed with its serial architecture strategy.

// NO_PILLOW_SUPPORT: Pillow 12.2 raises `conversion from LAB to RGB not supported`
// for ImageOps.grayscale(LAB). This native LAB-to-L route is a Rust extension.
// TODO(pillow-rs#3ptb): add target-only output and Architecture receipt evidence;
// Pillow parity cannot validate a route that the reference rejects.

use super::default::grayscale_impl;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::{
    ExecutionContext,
    registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, StrategyInputs,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Grayscale,
    Mode::ImageLab8,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    &[InputContract::CheckedByteDimensions {
        bytes_per_pixel: 1,
        allow_empty: true,
    }],
    &[],
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(grayscale_impl::Kernel<'a>),
}
pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: &'a grayscale_impl::Input,
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let implementation = grayscale_impl::prepare(input)?;
    let facts = StrategyInputs {
        width: input.width(),
        height: input.height(),
        byte_len: Some(input.as_raw().len()),
        palette: PaletteKnowledge::Unknown,
        request: Default::default(),
    };
    let selection = SUPPORT.select(ctx, facts)?;
    Ok(Prepared::new(
        ArchitectureStrategy::Default(implementation),
        selection,
    ))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(input) => {
            grayscale_impl::execute(input).map(DynamicImage::ImageLuma8)
        }
    }
}
