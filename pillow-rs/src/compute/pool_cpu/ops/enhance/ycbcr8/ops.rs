//! ImageYCbCr8 Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyInputs,
};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: CHANNELS,
    overflow_reason: "Brightness native dimensions overflowed",
    mismatch_reason: "Brightness source bytes did not match its dimensions",
}];
const CHANNELS: u8 = 3;
const ACTIVE_CHANNELS: u8 = 3;
const RIGHT_SHIFT_FACTORS: bool = false;

const ARCHITECTURES: [ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::enhance::helpers::strategy::byte_architectures(
        OperationId::Brightness,
        Mode::ImageYCbCr8,
        CHANNELS,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Brightness,
    Mode::ImageYCbCr8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
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
            .map(DynamicImage::ImageYCbCr8),
        Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
            crate::compute::pool_cpu::ops::enhance::helpers::vector::execute_image(
                source,
                CHANNELS,
                ACTIVE_CHANNELS,
                RIGHT_SHIFT_FACTORS,
                factor,
            )
            .map(DynamicImage::ImageYCbCr8)
        }
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Brightness mode {}",
            Mode::ImageYCbCr8.pillow_name().unwrap_or("internal"),
        ))),
    }
}


const CONTRAST_CHANNELS: u8 = 3;
const CONTRAST_INPUTS: &[crate::compute::registry::support::InputContract] =
    &[crate::compute::registry::support::InputContract::ExactNativeBytes {
        channels: CONTRAST_CHANNELS,
        overflow_reason: "Contrast native dimensions overflowed",
        mismatch_reason: "Contrast source bytes did not match its dimensions",
    }];

/// Exact-mode support for native Contrast blending.
pub(crate) const CONTRAST_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::serial(
        crate::compute::registry::support::OperationId::Contrast,
        crate::raster::Mode::ImageYCbCr8,
        CONTRAST_INPUTS,
    );

pub(crate) type ContrastStrategy<'a> =
    crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(
        &'a crate::raster::RgbImage,
        crate::compute::helpers::enhance::ContrastBase,
        f64,
    )>;

/// Select the native Contrast strategy for this exact mode.
pub(crate) fn prepare_contrast<'a>(
    source: &'a crate::raster::RgbImage,
    facts: crate::compute::registry::support::StrategyInputs,
    factor: f64,
    context: &crate::compute::ExecutionContext,
) -> Result<ContrastStrategy<'a>, crate::error::PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        CONTRAST_SUPPORT,
        facts,
        context,
    )?;
    let base = crate::compute::helpers::enhance::contrast_base_for_mode(
        crate::raster::Mode::ImageYCbCr8,
        source.as_raw(),
    )
    .ok_or_else(|| {
        crate::error::PilError::InternalError(
            "Contrast base was unavailable for a declared exact mode".into(),
        )
    })?;
    Ok(crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
        (source, base, factor),
        selection,
    ))
}

/// Execute the strategy selected during preparation without reclassifying mode.
pub(crate) fn execute_contrast(
    strategy: ContrastStrategy<'_>,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    let architecture = strategy.selection().implementation.architecture;
    let (source, base, factor) = strategy.implementation;
    match architecture {
        crate::compute::Architecture::Default => super::default::contrast_impl::execute(
            source,
            &base,
            factor,
        )
        .map(crate::raster::DynamicImage::ImageYCbCr8),
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Contrast mode {}",
            crate::raster::Mode::ImageYCbCr8.pillow_name().unwrap_or("internal"),
        ))),
    }
}

// Exact ImageYCbCr8 Sharpness strategy and native output binding.
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
        crate::raster::Mode::ImageYCbCr8,
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
            super::default::sharpness_impl::execute(source, factor).map(crate::raster::DynamicImage::ImageYCbCr8)
        }
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageYCbCr8.pillow_name().unwrap_or("internal"),
        ))),
    }
}
