//! I32 source-mode support and architecture selection for Convert.

use crate::compute::{Architecture, registry::support::{
    ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageI32,
    crate::compute::registry::support::OutputModeRule::ByTargetMode,
    &super::super::ops::OTHER_INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared I architecture".into(),
        )),
    }
}
