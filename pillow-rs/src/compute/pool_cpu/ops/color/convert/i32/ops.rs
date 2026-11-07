//! I32 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageI32
);

use crate::compute::{Architecture, registry::support::{
    InputContract, ModeSupport, OperationId, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// The declared I source/target row includes Pillow's L-derived results where
// a direct I converter is absent. I;16N clips through L before widening; this
// owner writes that final result without constructing an intermediate raster.
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::SCALAR_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageI32,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
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

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared I palette architecture".into(),
        )),
    }
}
