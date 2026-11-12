//! ImageRgba8 support and serial Default selection for ConvertLab.

pub(crate) use super::convert::SUPPORT as CONVERT_SUPPORT;
use crate::compute::{
    Architecture,
    registry::support::{ModeSupport, OperationId, OutputModeRule, StrategySelection},
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ConvertLab,
    Mode::ImageRgba8,
    OutputModeRule::Fixed(Mode::ImageLab8),
    super::super::ops::RGBA_INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbaImage,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_lab_impl::execute(source),
        _ => Err(PilError::InternalError(
            "ConvertLab selected an undeclared RGBA architecture".into(),
        )),
    }
}
