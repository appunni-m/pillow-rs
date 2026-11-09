use std::sync::Arc;

use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    PasteInputRules, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

pub(crate) type Strategy<'a> = BytePlan<'a>;

const RULES: PasteInputRules = PasteInputRules {
    source_modes: super::super::helpers::contracts::physical_byte_sources(Mode::ImageLumaA8),
    unmasked_only_sources: &[],
    compact_sources: super::super::helpers::contracts::compact_sources(Mode::ImageLumaA8),
    compact_masks: super::super::helpers::contracts::compact_masks(Mode::ImageLumaA8),
    masks: super::super::helpers::contracts::MASKS,
    source_rejection: "images do not match",
    mask_rejection: "bad transparency mask",
};
const INPUTS: &[InputContract] = &[InputContract::PasteInputs { rules: &RULES }];

static ARCHITECTURES: [ImplementationSupport; 4] =
    super::super::helpers::strategy::cpu_architectures_for_l_mask_neon(Mode::ImageLumaA8, 2, 8);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Paste,
    Mode::ImageLumaA8,
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
        destination,
        destination.dimensions(),
        Mode::ImageLumaA8,
        source,
        mask,
        mask_alpha,
        position,
        region_size,
        selection,
    )
}

pub(crate) fn execute<I>(strategy: BytePlan<'_, I>) -> Result<DynamicImage, PilError>
where
    I: std::ops::Deref<Target = DynamicImage>,
{
    let architecture = strategy.selection.implementation.architecture;
    super::super::helpers::plan::execute_with_hidden_lanes(
        strategy,
        |strategy| match architecture {
            Architecture::Default => super::default::paste_impl::execute(strategy),
            Architecture::Sse2 => super::sse2::paste_impl::execute(strategy),
            Architecture::Neon => super::neon::paste_impl::execute(strategy),
            Architecture::WasmSimd128 => super::wasm_simd128::paste_impl::execute(strategy),
            architecture => Err(PilError::InternalError(format!(
                "unbound {architecture:?} Paste strategy for mode {:?}",
                Mode::ImageLumaA8
            ))),
        },
    )
}
