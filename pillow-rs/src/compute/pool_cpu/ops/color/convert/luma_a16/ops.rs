//! Internal LA16 support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::high_depth::declare_native_binary_strategy!(
    crate::raster::Mode::ImageLumaA16,
    ImageLumaA16
);

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, ImageBuffer, LumaA, Mode};

type Source = ImageBuffer<LumaA<u16>, Vec<u16>>;

const TARGETS: &[Mode] = &[
    Mode::ImageLumaA16,
    Mode::One,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageLab8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

// NO_PILLOW_SUPPORT: LA16's native conversions and identity route are core-only.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Convert,
    Mode::ImageLumaA16,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
    "native conversion support for an internal raster mode",
);

pub(crate) fn execute(
    source: &Source,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA16 architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &Source,
    target: Mode,
    dither: Option<DitherMethod>,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target, dither)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA16 palette architecture".into(),
        )),
    }
}
