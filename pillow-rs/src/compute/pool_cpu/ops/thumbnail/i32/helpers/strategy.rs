use crate::compute::{Architecture, Backend};
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::Mode;

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
