use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn selected_architecture(
    selection: &StrategySelection,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::Thumbnail
        && implementation.mode == Mode::ImageI32
        && implementation.backend == Backend::Cpu
        && selection.requested_backend == Backend::Cpu
    {
        return Ok(implementation.architecture);
    }
    Err(PilError::InternalError(
        "ImageI32 Thumbnail received a mismatched CPU strategy selection".into(),
    ))
}
