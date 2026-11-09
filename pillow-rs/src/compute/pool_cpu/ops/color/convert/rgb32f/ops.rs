//! Internal RGB32F support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::high_depth::declare_native_binary_strategy!(
    crate::raster::Mode::ImageRgb32F,
    ImageRgb32F
);

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, ImageBuffer, Mode, Rgb};

type Source = ImageBuffer<Rgb<f32>, Vec<f32>>;

const TARGETS: &[Mode] = &[
    Mode::ImageRgb32F,
    Mode::ImageLumaA16,
    Mode::ImageRgb16,
    Mode::ImageRgba16,
    Mode::ImageRgba32F,
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

// NO_PILLOW_SUPPORT: RGB32F and direct cross-high-depth conversions are core-only.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Convert,
    Mode::ImageRgb32F,
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
            "Convert selected an undeclared RGB32F architecture".into(),
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
            "Convert selected an undeclared RGB32F palette architecture".into(),
        )),
    }
}
