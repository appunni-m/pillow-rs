//! YCbCr8 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageYCbCr8
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
}};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, Mode, RgbImage};

const TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLab8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// NO_PILLOW_SUPPORT: Pillow's YCbCr converters define only L, LA, and RGB.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageLab8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: Some("Pillow has no matching YCbCr source-target conversion"),
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageYCbCr8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared YCbCr8 architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &RgbImage,
    target: Mode,
    dither: Option<DitherMethod>,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target, dither)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared YCbCr palette architecture".into(),
        )),
    }
}
