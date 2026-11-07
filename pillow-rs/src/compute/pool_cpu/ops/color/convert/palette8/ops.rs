//! P source-mode support and architecture selection for Convert.

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OutputModeRule, PaletteInput,
        StrategySelection,
    },
};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayImage, Mode};

const TARGETS: &[Mode] = &[
    Mode::ImagePalette8,
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageRgbx8,
    Mode::ImageLab8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// Palette-to-LAB reads palette RGB entries in this mode strategy.
const CORE_ONLY_TARGETS: &[Mode] = &[];
const INPUTS: &[InputContract] = &[
    InputContract::PaletteMetadata,
    InputContract::ConvertTarget {
        targets: TARGETS,
        core_only_targets: CORE_ONLY_TARGETS,
        core_only_reason: None,
        rejection_reason: "image has wrong mode",
    },
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImagePalette8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute_binary(
    inputs: OperationInputs<'_>,
    _dither: Option<&DitherMethod>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    // Pillow thresholds palette entries directly before generic dithering.
    let DynamicImage::ImagePalette8(source) = inputs.image else {
        return Err(PilError::InternalError(
            "Convert selected P binary strategy for a different source mode".into(),
        ));
    };
    super::super::helpers::binary::palette_indices_to_binary(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel[0]),
        inputs.palette,
        selection,
    )
}

pub(crate) fn execute(
    source: &GrayImage,
    palette: PaletteInput<'_>,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, palette, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared P architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &GrayImage,
    palette: PaletteInput<'_>,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, palette, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared P palette architecture".into(),
        )),
    }
}
