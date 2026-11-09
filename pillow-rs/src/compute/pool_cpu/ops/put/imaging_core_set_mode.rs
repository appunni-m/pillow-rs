//! Support contract and Default strategy for callback-visible ImagingCore setmode.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationSupport, OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const IDENTITY_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedSetModeTargets { targets: &[] }];
const ONE_L_INPUTS: &[InputContract] = &[InputContract::CallerValidatedSetModeTargets {
    targets: &[Mode::One, Mode::ImageLuma8],
}];
const RGB_FAMILY_INPUTS: &[InputContract] = &[InputContract::CallerValidatedSetModeTargets {
    targets: &[Mode::ImageRgb8, Mode::ImageRgba8, Mode::ImageRgbx8],
}];

fn classify_mode(mode: Mode) -> ModeSupport {
    let inputs = match mode {
        Mode::One | Mode::ImageLuma8 => ONE_L_INPUTS,
        Mode::ImageRgb8 | Mode::ImageRgba8 | Mode::ImageRgbx8 => RGB_FAMILY_INPUTS,
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => IDENTITY_INPUTS,
    };
    if mode.is_pillow_mode() {
        ModeSupport::cpu(
            OperationId::ImagingCoreSetMode,
            mode,
            OutputModeRule::ByTargetMode,
            inputs,
            &[],
        )
    } else {
        // NO_PILLOW_SUPPORT: internal storage identities can only retain themselves.
        ModeSupport::cpu_core_only(
            OperationId::ImagingCoreSetMode,
            mode,
            OutputModeRule::ByTargetMode,
            inputs,
            &[],
            "internal raster identities have no Pillow ImagingCore setmode route",
        )
    }
}

pub(crate) static IMAGING_CORE_SET_MODE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::ImagingCoreSetMode, &[], classify_mode);

pub(crate) fn select(
    mode: Mode,
    target: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let support = IMAGING_CORE_SET_MODE_SUPPORT.for_mode(mode);
    support.validate_set_mode_target(target)?;
    support.select_serial(context)
}

pub(crate) fn execute(
    image: &mut DynamicImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<(), PilError> {
    if selection.implementation.operation != OperationId::ImagingCoreSetMode
        || selection.implementation.mode != image.mode()
        || selection.implementation.architecture != crate::compute::Architecture::Default
    {
        return Err(PilError::InternalError(
            "ImagingCore setmode strategy did not match its raster".into(),
        ));
    }
    super::helpers::imaging_core_mode::set_mode(image, target)
}
