use std::sync::Arc;

use crate::compute::pool_cpu::ops::paste::helpers::plan::NativeF32Plan;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, PasteInputRules, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, Rgba32FImage};

// NO_PILLOW_SUPPORT: this internal multichannel float identity has no Pillow Paste mode contract.
pub(crate) type Strategy<'a> = NativeF32Plan<'a>;

const RULES: PasteInputRules = PasteInputRules {
    source_modes: &[Mode::ImageRgba32F],
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
    Mode::ImageRgba32F,
    crate::compute::registry::support::OutputModeRule::Preserve,
    INPUTS,
    &[],
    "Paste on internal multichannel f32 storage has no Pillow mode contract",
);

pub(crate) fn prepare<'a>(
    destination: &'a Rgba32FImage,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    super::super::helpers::plan::native_f32_plan(
        destination.as_raw(),
        destination.dimensions(),
        Mode::ImageRgba32F,
        4,
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
