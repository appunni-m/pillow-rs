//! Wrapper-owned source/target compositions and their typed child transitions.

use crate::raster::Mode;

use super::{ModeSupport, OperationId, OperationRouteOutcome, OutputModeRule, SupportScope};

/// Identifies a wrapper-owned composition for one exact source/target pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationTargetCompositionKind {
    /// Reject during Convert's source/target validation before any child runs.
    ConvertGuard,
    /// Report a premultiplied target rejection when no direct mode strategy exists.
    ConvertPremultiplied,
    /// Convert to palette indices, then attach generated palette metadata.
    ConvertPalette,
}

/// Identifies the source used by an ordered target-composition step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationTransitionSource {
    /// Use the exact source raster supplied to the public operation.
    Input,
    /// Use the exact output raster from the preceding step.
    Previous,
}

/// One typed child-operation transition in a source/target composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationModeTransition {
    /// Child operation to prepare and execute.
    pub operation: OperationId,
    /// Exact source selection rule for this step.
    pub source: OperationTransitionSource,
    /// Exact destination identity requested from the child.
    pub target: Mode,
}

/// Wrapper-owned path for one exact source/target pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationTargetComposition {
    /// Exact input identity.
    pub source_mode: Mode,
    /// Exact output identity requested by the caller.
    pub target_mode: Mode,
    /// Wrapper algorithm that consumes the declared child transitions.
    pub kind: OperationTargetCompositionKind,
    /// Pillow-defined route or deliberate core-only extension.
    pub scope: SupportScope,
    /// Ordered child-operation mode transitions; the wrapper owns the final step.
    pub transitions: &'static [OperationModeTransition],
    /// Exact output contract of a successful composition.
    pub output: OutputModeRule,
    /// Existing result or wrapper/child rejection for this exact pair.
    pub outcome: OperationRouteOutcome,
    /// Prepared direct-operation admission available to the same source mode.
    pub prepared_support: ModeSupport,
}
