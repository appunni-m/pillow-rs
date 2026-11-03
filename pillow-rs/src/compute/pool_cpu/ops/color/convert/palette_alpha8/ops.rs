//! PA source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImagePaletteAlpha8
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, PaletteInput, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

const TARGETS: &[Mode] = &[
    Mode::ImagePaletteAlpha8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
    Mode::ImagePalette8,
    Mode::ImageRgb8,
    Mode::ImageLab8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// PA-to-LAB uses the palette-aware PA -> RGBA -> ImageCms route.
const CORE_ONLY_TARGETS: &[Mode] = &[
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageI16,
    Mode::ImageI16L,
    Mode::ImageI16B,
    Mode::ImageI16N,
];
const INPUTS: &[InputContract] = &[
    InputContract::PaletteMetadata,
    InputContract::ConvertTarget {
        targets: TARGETS,
        core_only_targets: CORE_ONLY_TARGETS,
        core_only_reason: Some("listed PA target routes still need Pillow-wrapper parity review"),
        rejection_reason: "image has wrong mode",
    },
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImagePaletteAlpha8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &GrayAlphaImage,
    palette: PaletteInput<'_>,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, palette, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared PA architecture".into(),
        )),
    }
}

pub(crate) fn execute_palette_target(
    source: &GrayAlphaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared PA palette architecture".into(),
        )),
    }
}
