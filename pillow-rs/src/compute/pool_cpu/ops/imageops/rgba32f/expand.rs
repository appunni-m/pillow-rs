//! ImageOps.expand strategy for exact Rgba32FImage storage.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, StrategyInputs,
};
use crate::compute::{ExecutionContext, pool_cpu::helpers::execution::prepared::Prepared};
use crate::error::PilError;
use crate::pipeline::NativeColorInput;
use crate::raster::{DynamicImage, Mode};

#[path = "default/expand_impl.rs"]
mod default;

const INPUTS: &[InputContract] = &[
    InputContract::ExpandedDimensions {
        bytes_per_pixel: 16,
    },
    InputContract::ExpandFill(crate::compute::registry::support::ExpandFillRule::NativeF32),
];
// NO_PILLOW_SUPPORT: this native Rgba32FImage expansion preserves internal
// sample storage and writes scalar/component fills as native f32 values.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Expand,
    Mode::ImageRgba32F,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
    "native high-depth border expansion is a core-only mode extension",
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::Kernel<'a>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn prepare<'a>(
    input: &'a default::Input,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    prepare_with_native_fill(input, border, fill, None, ctx)
}

pub(crate) fn prepare_with_native_fill<'a>(
    input: &'a default::Input,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    native_fill: Option<NativeColorInput>,
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
    let kernel = default::prepare(input, border, fill, native_fill);
    Ok(Prepared::new(
        ArchitectureStrategy::Default(kernel),
        selection,
    ))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(kernel) => {
            default::execute(kernel).map(DynamicImage::ImageRgba32F)
        }
    }
}
