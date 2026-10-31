use std::sync::Arc;

use crate::compute::pool_cpu::ops::paste::helpers::plan::NativeU16Plan;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, PasteInputRules, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode, LumaA};

// NO_PILLOW_SUPPORT: this internal high-depth identity has no Pillow Paste mode contract.
pub(crate) type Strategy<'a> = NativeU16Plan<'a>;

const RULES: PasteInputRules = PasteInputRules {
    source_modes: &[Mode::ImageLumaA16],
    unmasked_only_sources: &[],
    compact_sources: &[],
    compact_masks: &[],
    masks: super::super::helpers::contracts::MASKS,
    source_rejection: "images do not match",
    mask_rejection: "bad transparency mask",
};
const INPUTS: &[InputContract] = &[InputContract::PasteInputs { rules: &RULES }];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Paste,
    Mode::ImageLumaA16,
    crate::compute::registry::support::OutputModeRule::Preserve,
    INPUTS,
    &[],
    "Paste on internal high-depth u16 storage has no Pillow mode contract",
);

pub(crate) fn prepare<'a>(
    destination: &'a ImageBuffer<LumaA<u16>, Vec<u16>>,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    super::super::helpers::plan::native_u16_plan(
        destination.as_raw(),
        destination.dimensions(),
        Mode::ImageLumaA16,
        2,
        source,
        mask,
        mask_alpha,
        position,
        region_size,
        selection,
    )
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    super::default::paste_impl::execute(strategy)
}
