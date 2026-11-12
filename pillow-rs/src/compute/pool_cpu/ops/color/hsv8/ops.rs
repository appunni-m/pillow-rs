//! HSV8 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::helpers::convert::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageHsv8
);

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
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
    Mode::ImageRgbx8,
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
// This target row includes Pillow results derived through RGB; the HSV owner
// writes each final target from its native samples. The archived SIMD adapter
// has no exact HSV-source kernel; its generic three-byte route treats the
// stored H/S/V lanes as RGB and cannot be selected here.
const CORE_ONLY_TARGETS: &[Mode] = &[];
// TODO(pillow-rs#3ptb): evaluate exact HSV-source vector kernels after the
// Default source row has complete parity evidence.
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageHsv8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let mut output = match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared HSV8 architecture".into(),
        )),
    }?;
    super::super::helpers::convert::physical_lanes::preserve_compact_rgb_lane(source, target, &mut output)?;
    Ok(output)
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
            "Convert selected an undeclared HSV palette architecture".into(),
        )),
    }
}
