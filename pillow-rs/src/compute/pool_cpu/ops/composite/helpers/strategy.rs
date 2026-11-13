//! Shared guards for exact-mode Composite architecture selection.

use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, SecondaryInputFacts, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn select_support(
    support: ModeSupport,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
    operation: &'static str,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(format!(
            "{operation} preparation received unresolved input facts"
        ))),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn secondary_facts(image: &DynamicImage) -> SecondaryInputFacts {
    SecondaryInputFacts {
        mode: image.mode(),
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
    }
}

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
