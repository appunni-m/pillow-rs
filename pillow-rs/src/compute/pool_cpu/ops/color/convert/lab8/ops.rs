//! LAB8 source-mode support and architecture selection for Convert.

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

const TARGETS: &[Mode] = &[Mode::ImageLab8];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

// TODO(pillow-rs#3ptb): add the native LAB-to-color transform before admitting non-identity targets.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageLab8,
    OutputModeRule::ByTargetMode,
    INPUTS,
    &[],
);

pub(crate) fn execute(
    source: &RgbImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LAB8 architecture".into(),
        )),
    }
}
