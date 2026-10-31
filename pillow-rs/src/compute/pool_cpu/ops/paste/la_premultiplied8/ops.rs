use std::sync::Arc;

use crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, OutputModeRule,
    InputContract, ModeSupport, OperationId, PasteInputRules, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, GrayAlphaImage};

pub(crate) type Strategy<'a> = BytePlan<'a>;

const RULES: PasteInputRules = PasteInputRules {
    source_modes: &[Mode::ImageLaPremultiplied8],
    unmasked_only_sources: &[],
    compact_sources: super::super::helpers::contracts::compact_sources(Mode::ImageLaPremultiplied8),
    compact_masks: super::super::helpers::contracts::compact_masks(Mode::ImageLaPremultiplied8),
    masks: super::super::helpers::contracts::MASKS,
    source_rejection: "images do not match",
    mask_rejection: "bad transparency mask",
};
const INPUTS: &[InputContract] = &[InputContract::PasteInputs { rules: &RULES }];

static ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::cpu_architectures_for(Mode::ImageLaPremultiplied8, 2);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Paste,
    Mode::ImageLaPremultiplied8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn prepare<'a>(
    destination: &'a GrayAlphaImage,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    super::super::helpers::plan::byte_plan(
        destination.as_raw(),
        destination.dimensions(),
        Mode::ImageLaPremultiplied8,
        source,
        mask,
        mask_alpha,
        position,
        region_size,
        selection,
    )
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.selection.implementation.architecture {
        Architecture::Default => super::default::paste_impl::execute(strategy),
        Architecture::Sse2 => super::sse2::paste_impl::execute(strategy),
        Architecture::Neon => super::neon::paste_impl::execute(strategy),
        Architecture::WasmSimd128 => super::wasm_simd128::paste_impl::execute(strategy),
        architecture => Err(PilError::InternalError(format!(
            "unbound {architecture:?} Paste strategy for mode {:?}",
            Mode::ImageLaPremultiplied8
        ))),
    }
}
