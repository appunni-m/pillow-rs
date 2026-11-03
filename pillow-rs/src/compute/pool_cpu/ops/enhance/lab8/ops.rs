//! ImageLab8 Brightness contract and Default strategy.

use crate::compute::ExecutionContext;
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, RequestFacts,
    SecondaryInputFacts, StrategyInputs, StrategySelection,
};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Luma, Mode, RgbImage};

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

const CONTRAST_INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 3,
    overflow_reason: "Contrast native dimensions overflowed",
    mismatch_reason: "Contrast source bytes did not match its dimensions",
}];
pub(crate) const CONTRAST_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Contrast,
    Mode::ImageLab8,
    OutputModeRule::Preserve,
    CONTRAST_INPUTS,
    &[],
);

/// Prepared LAB Contrast route with every child implementation selected up front.
pub(crate) struct ContrastStrategy<'a> {
    source: &'a RgbImage,
    base: RgbImage,
    factor: f64,
    selection: StrategySelection,
    children: [StrategySelection; 4],
}

impl ContrastStrategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        &self.children
    }
}

/// Prepare LAB's archived L-mean, L-to-RGB-to-LAB base, and LAB blend routes.
pub(crate) fn prepare_contrast<'a>(
    source: &'a RgbImage,
    facts: StrategyInputs,
    factor: f64,
    context: &ExecutionContext,
) -> Result<ContrastStrategy<'a>, PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        CONTRAST_SUPPORT,
        facts.with_contrast(factor),
        context,
    )?;
    let (gray, lab_to_luma) = lab_contrast_luma(source, facts, context)?;
    let mean = rounded_mean(&gray);
    let constant_luma = GrayImage::from_pixel(gray.width(), gray.height(), Luma([mean]));
    let (rgb, luma_to_rgb) = luma_contrast_rgb(&constant_luma, context)?;
    let (base, rgb_to_lab) = rgb_contrast_lab(&rgb, context)?;
    let blend = blend_selection(&base, factor, facts, context)?;
    Ok(ContrastStrategy {
        source,
        base,
        factor,
        selection,
        children: [lab_to_luma, luma_to_rgb, rgb_to_lab, blend],
    })
}

fn lab_contrast_luma(
    source: &RgbImage,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<(GrayImage, StrategySelection), PilError> {
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        crate::compute::pool_cpu::ops::color::convert::lab8::ops::SUPPORT,
        facts.with_convert_target(Mode::ImageLuma8),
        context,
    )?;
    let image = crate::compute::pool_cpu::ops::color::convert::lab8::ops::execute(
        source,
        Mode::ImageLuma8,
        selection,
    )?;
    let DynamicImage::ImageLuma8(gray) = image else {
        return Err(PilError::InternalError(
            "LAB Contrast luma conversion returned another mode".into(),
        ));
    };
    Ok((gray, selection))
}

fn luma_contrast_rgb(
    source: &GrayImage,
    context: &ExecutionContext,
) -> Result<(RgbImage, StrategySelection), PilError> {
    let facts = StrategyInputs {
        width: source.width(),
        height: source.height(),
        byte_len: Some(source.as_raw().len()),
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::Unspecified,
    }
    .with_convert_target(Mode::ImageRgb8);
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        crate::compute::pool_cpu::ops::color::convert::luma8::ops::SUPPORT,
        facts,
        context,
    )?;
    let image = crate::compute::pool_cpu::ops::color::convert::luma8::ops::execute(
        source,
        Mode::ImageRgb8,
        selection,
    )?;
    let DynamicImage::ImageRgb8(rgb) = image else {
        return Err(PilError::InternalError(
            "LAB Contrast RGB base conversion returned another mode".into(),
        ));
    };
    Ok((rgb, selection))
}

fn rgb_contrast_lab(
    source: &RgbImage,
    context: &ExecutionContext,
) -> Result<(RgbImage, StrategySelection), PilError> {
    let facts = StrategyInputs {
        width: source.width(),
        height: source.height(),
        byte_len: Some(source.as_raw().len()),
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::Unspecified,
    };
    let selection = crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        crate::compute::pool_cpu::ops::color::ops::SUPPORT.for_mode(Mode::ImageRgb8),
        facts,
        context,
    )?;
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "ConvertLab selected an undeclared RGB architecture".into(),
        ));
    }
    let image = crate::compute::pool_cpu::ops::color::rgb8::default::convert_lab_impl::execute(
        source,
    )?;
    let DynamicImage::ImageLab8(lab) = image else {
        return Err(PilError::InternalError(
            "ConvertLab returned another mode during Contrast preparation".into(),
        ));
    };
    Ok((lab, selection))
}

fn blend_selection(
    base: &RgbImage,
    factor: f64,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let secondary = SecondaryInputFacts {
        mode: Mode::ImageLab8,
        width: base.width(),
        height: base.height(),
        byte_len: Some(base.as_raw().len()),
    };
    let facts = facts.with_blend_inputs(secondary, factor);
    crate::compute::pool_cpu::ops::enhance::helpers::strategy::select(
        crate::compute::pool_cpu::ops::composite::ops::BLEND_MODULE_SUPPORT
            .for_mode(Mode::ImageLab8),
        facts,
        context,
    )
}

fn rounded_mean(gray: &GrayImage) -> u8 {
    let samples = gray.as_raw();
    if samples.is_empty() {
        return 0;
    }
    let sum: u64 = samples.iter().map(|&sample| u64::from(sample)).sum();
    (sum as f64 / samples.len() as f64 + 0.5) as u8
}

pub(crate) fn execute_contrast(
    strategy: ContrastStrategy<'_>,
) -> Result<DynamicImage, PilError> {
    if strategy.selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "LAB Contrast selected an undeclared architecture".into(),
        ));
    }
    super::default::contrast_impl::execute(
        strategy.source,
        &strategy.base,
        strategy.factor,
        &strategy.children[3],
    )
}

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
    let mut output = match architecture {
        crate::compute::Architecture::Default => {
            super::default::sharpness_impl::execute(source, factor).map(crate::raster::DynamicImage::ImageLab8)
        }
        architecture => Err(crate::error::PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Sharpness mode {}",
            crate::raster::Mode::ImageLab8.pillow_name().unwrap_or("internal"),
        ))),
    }?;
    crate::compute::pool_cpu::ops::enhance::helpers::sharpness::carry_hidden_lanes(
        source,
        &mut output,
        crate::raster::Mode::ImageLab8,
        factor,
    )?;
    Ok(output)
}
