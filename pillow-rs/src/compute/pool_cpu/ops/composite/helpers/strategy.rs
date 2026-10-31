//! Shared guards for exact-mode Composite architecture selection.

use crate::compute::Architecture;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn require_operation_mode(
    selection: &StrategySelection,
    operation: OperationId,
    mode: Mode,
) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.operation != operation || implementation.mode != mode {
        return Err(PilError::InternalError(format!(
            "unbound {operation:?} architecture strategy for mode {mode:?}"
        )));
    }
    Ok(())
}

pub(crate) fn require_default(
    selection: &StrategySelection,
    operation: OperationId,
    mode: Mode,
) -> Result<(), PilError> {
    require_operation_mode(selection, operation, mode)?;
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(format!(
            "unbound {operation:?} architecture strategy for mode {mode:?}"
        )));
    }
    Ok(())
}
