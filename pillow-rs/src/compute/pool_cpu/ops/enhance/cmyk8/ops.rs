//! ImageCmyk8 Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyInputs,
};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: CHANNELS,
    overflow_reason: "Brightness native dimensions overflowed",
    mismatch_reason: "Brightness source bytes did not match its dimensions",
}];
const CHANNELS: u8 = 4;
const ACTIVE_CHANNELS: u8 = 4;
const RIGHT_SHIFT_FACTORS: bool = false;

const COLOR_SATURATION_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 4,
    overflow_reason: "ColorSaturation native dimensions overflowed",
    mismatch_reason: "ColorSaturation source bytes did not match its dimensions",
}];
const COLOR_SATURATION_ARCHITECTURES: [ImplementationSupport; 2] =
    crate::compute::pool_cpu::ops::enhance::helpers::strategy::color_saturation_architectures(
        OperationId::ColorSaturation,
        Mode::ImageCmyk8,
        4,
    );

const ARCHITECTURES: [ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::enhance::helpers::strategy::byte_architectures(
        OperationId::Brightness,
        Mode::ImageCmyk8,
        CHANNELS,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Brightness,
    Mode::ImageCmyk8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) const COLOR_SATURATION_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ColorSaturation,
    Mode::ImageCmyk8,
    OutputModeRule::Preserve,
    COLOR_SATURATION_INPUTS,
    &COLOR_SATURATION_ARCHITECTURES,
);

pub(crate) type Strategy<'a> = Prepared<(&'a RgbaImage, f64)>;

pub(crate) fn prepare<'a>(
    source: &'a RgbaImage,
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
            .map(DynamicImage::ImageCmyk8),
        Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
            crate::compute::pool_cpu::ops::enhance::helpers::vector::execute_image(
                source,
                CHANNELS,
                ACTIVE_CHANNELS,
                RIGHT_SHIFT_FACTORS,
                factor,
            )
            .map(DynamicImage::ImageCmyk8)
        }
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Brightness mode {}",
            Mode::ImageCmyk8.pillow_name().unwrap_or("internal"),
        ))),
    }
}

pub(crate) fn prepare_color_saturation<'a>(
    source: &'a RgbaImage,
    facts: StrategyInputs,
    factor: f64,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        COLOR_SATURATION_SUPPORT,
        facts,
        context,
    )?;
    Ok(Prepared::new((source, factor), selection))
}

pub(crate) fn execute_color_saturation(
    strategy: Strategy<'_>,
) -> Result<DynamicImage, PilError> {
    let architecture = strategy.selection().implementation.architecture;
    let (source, factor) = strategy.implementation;
    match architecture {
        Architecture::Default => super::default::color_saturation_impl::execute(source, factor)
            .map(DynamicImage::ImageCmyk8),
        Architecture::AvxFma => super::avx::color_saturation_impl::execute(source, factor)
            .map(DynamicImage::ImageCmyk8),
        Architecture::Neon => super::neon::color_saturation_impl::execute(source, factor)
            .map(DynamicImage::ImageCmyk8),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for ColorSaturation mode {}",
            Mode::ImageCmyk8.pillow_name().unwrap_or("internal"),
        ))),
    }
}


const CONTRAST_CHANNELS: u8 = 4;
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
        crate::raster::Mode::ImageCmyk8,
        CONTRAST_INPUTS,
    );

pub(crate) type ContrastStrategy<'a> =
    crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(
        &'a crate::raster::RgbaImage,
        crate::compute::helpers::enhance::ContrastBase,
        f64,
    )>;

/// Select the native Contrast strategy for this exact mode.
pub(crate) fn prepare_contrast<'a>(
    source: &'a crate::raster::RgbaImage,
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
        crate::raster::Mode::ImageCmyk8,
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
        .map(crate::raster::DynamicImage::ImageCmyk8),
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Contrast mode {}",
            crate::raster::Mode::ImageCmyk8.pillow_name().unwrap_or("internal"),
        ))),
    }
}

// Exact ImageCmyk8 Sharpness strategy and native output binding.
const SHARPNESS_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::SharpnessFactor,
    crate::compute::registry::support::InputContract::ExactNativeBytes {
        channels: 4,
        overflow_reason: "Sharpness native dimensions overflowed",
        mismatch_reason: "Sharpness source bytes did not match its dimensions",
    },
];
const SHARPNESS_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::enhance::helpers::strategy::sharpness_architectures(
        crate::compute::registry::support::OperationId::Sharpness,
        crate::raster::Mode::ImageCmyk8,
        4,
    );
pub(crate) const SHARPNESS_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::cpu(
        crate::compute::registry::support::OperationId::Sharpness,
        crate::raster::Mode::ImageCmyk8,
        crate::compute::registry::support::OutputModeRule::Preserve,
        SHARPNESS_INPUTS,
        &SHARPNESS_ARCHITECTURES,
    );

pub(crate) type SharpnessStrategy<'a> = crate::compute::pool_cpu::helpers::execution::prepared::Prepared<(&'a crate::raster::RgbaImage, f64)>;

pub(crate) fn prepare_sharpness<'a>(
    source: &'a crate::raster::RgbaImage,
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
            super::default::sharpness_impl::execute(source, factor)
                .map(crate::raster::DynamicImage::ImageCmyk8)
        }
        crate::compute::Architecture::Avx2Fma => super::avx2::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::Avx2 => super::avx2::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::AvxFma => super::avx::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::Avx => super::avx::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::Sse2 => super::sse2::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::Neon => super::neon::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        crate::compute::Architecture::WasmSimd128 => super::wasm_simd128::sharpness_impl::execute(source, factor)
            .map(crate::raster::DynamicImage::ImageCmyk8),
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageCmyk8.pillow_name().unwrap_or("internal"),
        ))),
    }
}
