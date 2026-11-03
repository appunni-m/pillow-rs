//! I32 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageI32
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// NO_PILLOW_SUPPORT: Pillow's I converters omit these exact target pairs.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLumaA8,
    Mode::ImageI16N,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageLab8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
];
// TODO(pillow-rs#3ptb): add CoreOnly CPU parity for I→I;16N with negative,
// above-u16, >255, and in-range samples; Pillow has no I-to-I;16N converter.
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::SCALAR_TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: Some("Pillow has no matching I source-target conversion"),
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageI32,
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
            "Convert selected an undeclared I architecture".into(),
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
            "Convert selected an undeclared I palette architecture".into(),
        )),
    }
}
