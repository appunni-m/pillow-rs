//! P source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImagePalette8
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, PaletteInput, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const TARGETS: &[Mode] = &[
    Mode::ImagePalette8,
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageLab8,
    Mode::ImageRgba8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
// TODO(pillow-rs#3ptb): Pillow routes palette-to-LAB through RGBA and ImageCms.
// Keep this CoreOnly until the Rust composition matches that public contract.
const CORE_ONLY_TARGETS: &[Mode] = &[Mode::ImageLab8];
const INPUTS: &[InputContract] = &[
    InputContract::PaletteMetadata,
    InputContract::ConvertTarget {
        targets: TARGETS,
        core_only_targets: CORE_ONLY_TARGETS,
        core_only_reason: Some("Rust P-to-LAB has not completed ImageCms parity review"),
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
