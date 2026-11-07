//! ImageI16N Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyInputs,
};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Mode};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 2,
    overflow_reason: "Brightness native dimensions overflowed",
    mismatch_reason: "Brightness source bytes did not match its dimensions",
}];

// NO_PILLOW_SUPPORT: Pillow's Brightness path reaches Image.blend, which rejects
// non-u8 storage; this is a native u16-sample Rust extension.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Brightness,
    Mode::ImageI16N,
    OutputModeRule::Preserve,
    INPUTS,
    &[],
    "native u16 Brightness extends Pillow's byte-only Image.blend contract",
);

pub(crate) type Strategy<'a> = Prepared<(&'a ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, f64)>;

pub(crate) fn prepare<'a>(
    source: &'a ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
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
            .map(DynamicImage::ImageI16N),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Brightness mode {}",
            Mode::ImageI16N.pillow_name().unwrap_or("internal"),
        ))),
    }
}

// Exact ImageI16N Sharpness strategy and native output binding.
const SHARPNESS_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::SharpnessFactor,
    crate::compute::registry::support::InputContract::ExactNativeBytes {
        channels: 2,
        overflow_reason: "Sharpness native dimensions overflowed",
        mismatch_reason: "Sharpness source bytes did not match its dimensions",
    },
];
// NO_PILLOW_SUPPORT: this exact-mode Sharpness route is a native core extension.
pub(crate) const SHARPNESS_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::cpu_core_only(
        crate::compute::registry::support::OperationId::Sharpness,
        crate::raster::Mode::ImageI16N,
        crate::compute::registry::support::OutputModeRule::Preserve,
        SHARPNESS_INPUTS,
        &[],
        "native Sharpness extension has no Pillow parity contract",
    );

pub(crate) type SharpnessStrategy<'a> = crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(&'a crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>, f64)>;

pub(crate) fn prepare_sharpness<'a>(
    source: &'a crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
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
            super::default::sharpness_impl::execute(source, factor).map(crate::raster::DynamicImage::ImageI16N)
        }
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageI16N.pillow_name().unwrap_or("internal"),
        ))),
    }
}
