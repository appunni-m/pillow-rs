//! ImageRgb32F Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs,
};
use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, Rgb32FImage};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 12,
    overflow_reason: "Brightness native dimensions overflowed",
    mismatch_reason: "Brightness source bytes did not match its dimensions",
}];

// NO_PILLOW_SUPPORT: internal RGB32F Brightness is a native Rust extension
// with no Pillow mode identity.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Brightness,
    Mode::ImageRgb32F,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
    "internal high-depth raster support has no Pillow parity contract",
);

pub(crate) type Strategy<'a> = Prepared<(&'a Rgb32FImage, f64)>;

pub(crate) fn prepare<'a>(
    source: &'a Rgb32FImage,
    facts: StrategyInputs,
    factor: f64,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        SUPPORT,
        facts,
        context,
    )?;
    Ok(Prepared::new((source, factor), selection))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    let architecture = strategy.selection().implementation.architecture;
    let (source, factor) = strategy.implementation;
    match architecture {
        Architecture::Default => super::default::brightness_impl::execute(source, factor)
            .map(DynamicImage::ImageRgb32F),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Brightness mode {}",
            Mode::ImageRgb32F.pillow_name().unwrap_or("internal"),
        ))),
    }
}

// Exact ImageRgb32F Sharpness strategy and native output binding.
const SHARPNESS_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::SharpnessFactor,
    crate::compute::registry::support::InputContract::ExactNativeBytes {
        channels: 12,
        overflow_reason: "Sharpness native dimensions overflowed",
        mismatch_reason: "Sharpness source bytes did not match its dimensions",
    },
];
// NO_PILLOW_SUPPORT: this exact-mode Sharpness route is a native core extension.
pub(crate) const SHARPNESS_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::cpu_core_only(
        crate::compute::registry::support::OperationId::Sharpness,
        crate::raster::Mode::ImageRgb32F,
        crate::compute::registry::support::OutputModeRule::Preserve,
        SHARPNESS_INPUTS,
        &[],
        "native Sharpness extension has no Pillow parity contract",
    );

pub(crate) type SharpnessStrategy<'a> = crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(&'a crate::raster::Rgb32FImage, f64)>;

pub(crate) fn prepare_sharpness<'a>(
    source: &'a crate::raster::Rgb32FImage,
    facts: crate::compute::registry::support::StrategyInputs,
    factor: f64,
    context: &crate::compute::ExecutionContext,
) -> Result<SharpnessStrategy<'a>, crate::error::PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        SHARPNESS_SUPPORT, facts.with_sharpness(factor), context,
    )?;
    Ok(crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
        (source, factor), selection,
    ))
}

pub(crate) fn execute_sharpness(
    strategy: SharpnessStrategy<'_>,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    let architecture = strategy.selection().implementation.architecture;
    let (source, factor) = strategy.implementation;
    match architecture {
        crate::compute::Architecture::Default => {
            super::default::sharpness_impl::execute(source, factor).map(crate::raster::DynamicImage::ImageRgb32F)
        }
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageRgb32F.pillow_name().unwrap_or("internal"),
        ))),
    }
}
