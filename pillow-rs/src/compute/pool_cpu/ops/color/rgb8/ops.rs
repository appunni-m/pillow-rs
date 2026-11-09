//! ImageRgb8 support and serial Default selection for ConvertLab.

use crate::compute::{
    Architecture,
    registry::support::{
        InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

const INPUTS: &[InputContract] = super::super::ops::RGB_INPUTS;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ConvertLab,
    Mode::ImageRgb8,
    OutputModeRule::Fixed(Mode::ImageLab8),
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbImage,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_lab_impl::execute(source),
        _ => Err(PilError::InternalError(
            "ConvertLab selected an undeclared RGB architecture".into(),
        )),
    }
}
