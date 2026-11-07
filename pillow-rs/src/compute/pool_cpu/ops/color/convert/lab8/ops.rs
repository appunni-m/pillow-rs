//! LAB8 source-mode support and architecture selection for Convert.

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OutputModeRule, StrategySelection,
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
    Mode::ImageRgb8,
    Mode::ImageRgbx8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgba8,
    Mode::ImageLab8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// TODO(pillow-rs#3ptb): verify Pillow-supported and CoreOnly LAB target
// outputs, ImageCms routes, error behavior, and selection receipts.
// TODO(pillow-rs#3ptb): assess an architecture-specific LAB LUT kernel only
// after it can preserve exact RGB/RGBA/RGBX output bytes.
// NO_PILLOW_SUPPORT: Pillow's LAB wrapper exposes identity and ImageCms
// conversion to RGB, RGBA, and RGBX; the remaining native outputs extend it.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: Some("Pillow's LAB wrapper only exposes RGB-family ImageCms targets"),
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLab8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute_binary(
    inputs: OperationInputs<'_>,
    dither: Option<&DitherMethod>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "Convert selected an undeclared LAB binary architecture".into(),
        ));
    }
    if inputs.mode() != Mode::ImageLab8 {
        return Err(PilError::InternalError(
            "Convert LAB binary strategy received a different source mode".into(),
        ));
    }
    let DynamicImage::ImageLab8(source) = inputs.image else {
        return Err(PilError::InternalError(
            "Convert LAB binary strategy received a different image carrier".into(),
        ));
    };
    super::default::convert_impl::execute_binary(source, dither)
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
            "Convert selected an undeclared LAB palette architecture".into(),
        )),
    }
}

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let mut output = match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LAB8 architecture".into(),
        )),
    }?;
    super::super::helpers::physical_lanes::preserve_compact_rgb_lane(source, target, &mut output)?;
    Ok(output)
}
