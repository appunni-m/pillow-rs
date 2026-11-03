use crate::compute::{Architecture, Backend};
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn selected_architecture(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::Draw
        && implementation.mode == mode
        && implementation.backend == Backend::Cpu
        && selection.requested_backend == Backend::Cpu
    {
        return Ok(implementation.architecture);
    }
    Err(PilError::InternalError(
        "Draw mode received a mismatched CPU strategy selection".into(),
    ))
}

pub(crate) fn require_default_selection(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<(), PilError> {
    if selected_architecture(selection, mode)? == Architecture::Default {
        return Ok(());
    }
    Err(PilError::InternalError(
        "Draw Default mode received a non-Default strategy selection".into(),
    ))
}
