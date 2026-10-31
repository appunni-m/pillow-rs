//! ImageLab8 Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs,
};
use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 3,
    overflow_reason: "Brightness native dimensions overflowed",
    mismatch_reason: "Brightness source bytes did not match its dimensions",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Brightness,
    Mode::ImageLab8,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
);

pub(crate) type Strategy<'a> = Prepared<(&'a RgbImage, f64)>;

pub(crate) fn prepare<'a>(
    source: &'a RgbImage,
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
            .map(DynamicImage::ImageLab8),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Brightness mode {}",
            Mode::ImageLab8.pillow_name().unwrap_or("internal"),
        ))),
    }
}

// Exact ImageLab8 Sharpness strategy and native output binding.
const SHARPNESS_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::SharpnessFactor,
    crate::compute::registry::support::InputContract::ExactNativeBytes {
        channels: 3,
        overflow_reason: "Sharpness native dimensions overflowed",
        mismatch_reason: "Sharpness source bytes did not match its dimensions",
    },
];
// NO_PILLOW_SUPPORT: this exact-mode Sharpness route is a native core extension.
pub(crate) const SHARPNESS_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::cpu_core_only(
        crate::compute::registry::support::OperationId::Sharpness,
        crate::raster::Mode::ImageLab8,
        crate::compute::registry::support::OutputModeRule::Preserve,
        SHARPNESS_INPUTS,
        &[],
        "native Sharpness extension has no Pillow parity contract",
    );

pub(crate) type SharpnessStrategy<'a> = crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(&'a crate::raster::RgbImage, f64)>;

pub(crate) fn prepare_sharpness<'a>(
    source: &'a crate::raster::RgbImage,
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
            super::default::sharpness_impl::execute(source, factor).map(crate::raster::DynamicImage::ImageLab8)
        }
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageLab8.pillow_name().unwrap_or("internal"),
        ))),
    }
}
