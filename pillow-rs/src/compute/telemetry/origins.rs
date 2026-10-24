//! Public workflow provenance attached to operation telemetry.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

/// Identifies the public workflow call that created an operation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineTelemetryOrigin {
    /// Workflow step ID from the selected benchmark input.
    pub step_id: String,
    /// Public API surface from the selected workflow step.
    pub surface: String,
    /// Public operation from the selected workflow step.
    pub operation: String,
}

type OperationOrigins = BTreeMap<&'static str, VecDeque<Option<PipelineTelemetryOrigin>>>;

thread_local! {
    static ACTIVE_ORIGIN: RefCell<Option<PipelineTelemetryOrigin>> = const { RefCell::new(None) };
    static ACTIVE_OPERATION_ORIGINS: RefCell<Option<OperationOrigins>> = const { RefCell::new(None) };
}

/// Restore the previous lazy-pipeline origin queue after execution.
pub(crate) struct RestorePipelineOperationOrigins(Option<OperationOrigins>);

impl Drop for RestorePipelineOperationOrigins {
    fn drop(&mut self) {
        ACTIVE_OPERATION_ORIGINS.with(|active| {
            *active.borrow_mut() = self.0.take();
        });
    }
}

pub(crate) fn replace_active_origin(
    origin: Option<PipelineTelemetryOrigin>,
) -> Option<PipelineTelemetryOrigin> {
    ACTIVE_ORIGIN.with(|active| {
        let mut active = active.borrow_mut();
        match origin {
            Some(origin) => active.replace(origin),
            None => active.take(),
        }
    })
}

pub(crate) fn capture_active_origin() -> Option<PipelineTelemetryOrigin> {
    ACTIVE_ORIGIN.with(|active| active.borrow().clone())
}

pub(crate) fn origin_for_event(operation: &'static str) -> Option<PipelineTelemetryOrigin> {
    let queued_origin = ACTIVE_OPERATION_ORIGINS.with(|active| {
        let mut active = active.borrow_mut();
        let origins = active.as_mut()?;
        Some(
            origins
                .get_mut(operation)
                .and_then(VecDeque::pop_front)
                .flatten(),
        )
    });
    queued_origin.unwrap_or_else(capture_active_origin)
}

pub(crate) fn enter_operation_origins(
    bindings: impl IntoIterator<Item = (&'static str, Option<PipelineTelemetryOrigin>)>,
) -> RestorePipelineOperationOrigins {
    let mut origins = OperationOrigins::new();
    for (operation, origin) in bindings {
        origins.entry(operation).or_default().push_back(origin);
    }
    let previous = ACTIVE_OPERATION_ORIGINS.with(|active| {
        let mut active = active.borrow_mut();
        let previous = active.take();
        *active = Some(origins);
        previous
    });
    RestorePipelineOperationOrigins(previous)
}
