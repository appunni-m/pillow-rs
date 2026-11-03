use super::super::ops::{FilterRequest, INTERNAL_MODE, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ModeSupport, OperationId, RejectionKind, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageI16L;

pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 | OperationId::Filter5x5 => pillow_kernel(operation, MODE, &[]),
        _ => ModeSupport::Rejected {
            mode: MODE,
            reason: INTERNAL_MODE,
            kind: RejectionKind::ValueError,
        },
    }
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError("I;16L selected an unowned architecture".into()));
    }
    super::default::execute(image, request)
}
