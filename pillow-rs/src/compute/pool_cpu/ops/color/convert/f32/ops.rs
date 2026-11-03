//! F32 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageF32
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// NO_PILLOW_SUPPORT: Pillow's F converters implement only L and I outputs.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLumaA8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageLab8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::SCALAR_TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: Some("Pillow has no matching F source-target conversion"),
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageF32,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared F architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared F palette architecture".into(),
        )),
    }
}
