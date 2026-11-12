//! Mode-level strategy support and selection.

use super::{
    Architecture, Backend, ExecutionContext, FallbackReason, GuardAssessment,
    ImplementationSupport, InputContract, Mode, ModeSupport, OperationId, OperationValueResult,
    OutputModeRule, PaletteKnowledge, PilError, RequestFacts, StrategyGuard, StrategyInputs,
    StrategySelection, SupportAssessment, SupportScope, UnresolvedGuard,
    prioritized_implementations,
};

impl ModeSupport {
    /// Adds the value-result shape declared by this mode strategy.
    #[must_use]
    pub const fn with_value_result(self, value_result: OperationValueResult) -> Self {
        match self {
            Self::Supported {
                scope,
                default,
                architectures,
                output,
                inputs,
                ..
            } => Self::Supported {
                scope,
                default,
                architectures,
                output,
                inputs,
                value_result: Some(value_result),
            },
            rejected @ Self::Rejected { .. } => rejected,
        }
    }

    /// Returns the declared non-raster value-result shape for this exact mode.
    #[must_use]
    pub const fn value_result(self) -> Option<OperationValueResult> {
        match self {
            Self::Supported { value_result, .. } => value_result,
            Self::Rejected { .. } => None,
        }
    }

    /// Rejects unsupported source modes through this operation's declaration.
    pub(crate) fn validate_mode(self) -> Result<(), PilError> {
        match self {
            Self::Supported { .. } => Ok(()),
            Self::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        }
    }

    /// Validates a target identity against a mode-changing operation contract.
    pub(crate) fn validate_set_mode_target(self, target: Mode) -> Result<(), PilError> {
        let Self::Supported {
            default, inputs, ..
        } = self
        else {
            return self.validate_mode();
        };
        for contract in inputs {
            if let InputContract::CallerValidatedSetModeTargets { targets } = contract {
                return if target == default.id.mode || targets.contains(&target) {
                    Ok(())
                } else {
                    Err(PilError::ValueError("image has wrong mode".into()))
                };
            }
        }
        Err(PilError::InternalError(
            "setmode support omitted its target-mode contract".into(),
        ))
    }

    /// Validates a Blend secondary identity from this mode's declaration.
    ///
    /// The public wrapper calls this before querying dimensions so unsupported
    /// modes keep their historical error ordering without maintaining a
    /// second mode-pair matrix beside `BlendInputs`.
    pub(crate) fn validate_blend_input_mode(self, other_mode: Mode) -> Result<(), PilError> {
        self.validate_mode()?;
        let Self::Supported { inputs, .. } = self else {
            unreachable!("validated mode support was rejected")
        };
        for contract in inputs {
            let InputContract::BlendInputs {
                compatible_modes,
                wrong_mode_modes,
                wrong_mode_rejection,
                mismatch_rejection,
            } = *contract
            else {
                continue;
            };
            if compatible_modes.contains(&other_mode) {
                return Ok(());
            }
            let reason = if wrong_mode_modes.contains(&other_mode) {
                wrong_mode_rejection
            } else {
                mismatch_rejection
            };
            return Err(PilError::ValueError(reason.into()));
        }
        Err(PilError::InternalError(
            "Blend support omitted its secondary-mode contract".into(),
        ))
    }

    /// Resolves Paste's mask channel from this mode's declared input contract.
    ///
    /// Returns `true` when the accepted mask supplies alpha and `false` when it
    /// supplies luma. The public Paste wrapper uses this before source
    /// normalization so it does not maintain a second accepted-mask table.
    pub(crate) fn paste_mask_uses_alpha(self, mask_mode: Mode) -> Result<bool, PilError> {
        let inputs = match self {
            Self::Supported { inputs, .. } => inputs,
            Self::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        };
        for contract in inputs {
            let InputContract::PasteInputs { rules } = *contract else {
                continue;
            };
            return rules
                .masks
                .iter()
                .find(|rule| rule.mode == mask_mode)
                .map(|rule| rule.alpha)
                .ok_or_else(|| PilError::ValueError(rules.mask_rejection.into()));
        }
        Err(PilError::InternalError(
            "Paste support omitted its mask contract".into(),
        ))
    }

    /// Validates an ImageChops secondary identity from this mode's declaration.
    ///
    /// The secondary identity is supplied lazily so source rejection can
    /// precede materializing or inspecting the secondary image when required.
    pub(crate) fn validate_chops_secondary_mode(
        self,
        other_mode: impl FnOnce() -> Result<Mode, PilError>,
    ) -> Result<(), PilError> {
        self.validate_mode()?;
        let other_mode = other_mode()?;
        let Self::Supported { inputs, .. } = self else {
            unreachable!("validated Chops source mode was rejected")
        };
        for contract in inputs {
            let InputContract::ChopsBinaryModes {
                modes,
                rejection_reason,
                ..
            } = *contract
            else {
                continue;
            };
            return if modes.contains(&other_mode) {
                Ok(())
            } else {
                Err(PilError::ValueError(rejection_reason.into()))
            };
        }
        Err(PilError::InternalError(
            "Chops support omitted its secondary-mode contract".into(),
        ))
    }

    /// Checks declared input contracts without selecting an architecture.
    ///
    /// A returned guard means the caller does not yet have enough facts to
    /// complete admission; materialized preparation resolves it later.
    pub(crate) fn assess_inputs(
        self,
        input: StrategyInputs,
    ) -> Result<Option<UnresolvedGuard>, PilError> {
        self.validate_mode()?;
        let Self::Supported {
            default, inputs, ..
        } = self
        else {
            unreachable!("validated mode support was rejected")
        };
        let palette_identity_copy = default.id.operation == OperationId::Convert
            && matches!(
                input.request,
                RequestFacts::ConvertTarget { target_mode }
                    if target_mode == default.id.mode
            );
        let mut unresolved = None;
        for contract in inputs {
            // A same-mode P/PA Convert copies its indexed raster and carries
            // wrapper metadata unchanged; it does not need resolved palette
            // colors to choose or execute the identity strategy.
            if palette_identity_copy && matches!(contract, InputContract::PaletteMetadata) {
                continue;
            }
            if let Some(guard) = contract.assess(input)? {
                if unresolved.is_none() {
                    unresolved = Some(guard);
                }
            }
        }
        Ok(unresolved)
    }

    /// Returns whether this mode route is Pillow-defined or a core extension.
    #[must_use]
    pub const fn scope(self) -> Option<SupportScope> {
        match self {
            Self::Supported { scope, .. } => Some(scope),
            Self::Rejected { .. } => None,
        }
    }

    /// Returns the declared scope of an accepted conversion target pair.
    #[must_use]
    pub fn scope_for_target(self, target: Mode) -> Option<SupportScope> {
        let Self::Supported { scope, inputs, .. } = self else {
            return None;
        };
        for input in inputs {
            if let InputContract::ConvertTarget {
                targets,
                core_only_targets,
                core_only_reason,
                ..
            } = input
            {
                if !targets.contains(&target) {
                    return None;
                }
                if let SupportScope::CoreOnly { .. } = scope {
                    return Some(scope);
                }
                if core_only_targets.contains(&target) || !target.is_pillow_mode() {
                    return Some(SupportScope::CoreOnly {
                        reason: (*core_only_reason)
                            .unwrap_or("this source/target pair is a native core extension"),
                    });
                }
                return Some(SupportScope::Pillow);
            }
        }
        None
    }

    /// Declares a concrete serial strategy with no vector specialization.
    pub const fn serial(
        operation: OperationId,
        mode: Mode,
        inputs: &'static [InputContract],
    ) -> Self {
        Self::cpu(operation, mode, OutputModeRule::Preserve, inputs, &[])
    }

    /// Declares a Pillow-defined strategy for this exact operation and mode.
    ///
    /// Use this only when Pillow defines the operation/mode behavior. A Pillow
    /// spelling on `mode` does not establish that the operation supports it;
    /// Rust-only extensions must use [`Self::cpu_core_only`] with a nearby
    /// `NO_PILLOW_SUPPORT` comment at the implementation declaration.
    pub const fn cpu(
        operation: OperationId,
        mode: Mode,
        output: OutputModeRule,
        inputs: &'static [InputContract],
        architectures: &'static [ImplementationSupport],
    ) -> Self {
        let scope = if mode.is_pillow_mode() {
            SupportScope::Pillow
        } else {
            SupportScope::CoreOnly {
                reason: "this internal raster identity has no Pillow mode spelling",
            }
        };
        Self::cpu_with_scope(operation, mode, output, inputs, architectures, scope)
    }

    /// Declares a native strategy that extends Pillow's operation/mode contract.
    pub const fn cpu_core_only(
        operation: OperationId,
        mode: Mode,
        output: OutputModeRule,
        inputs: &'static [InputContract],
        architectures: &'static [ImplementationSupport],
        reason: &'static str,
    ) -> Self {
        Self::cpu_with_scope(
            operation,
            mode,
            output,
            inputs,
            architectures,
            SupportScope::CoreOnly { reason },
        )
    }

    const fn cpu_with_scope(
        operation: OperationId,
        mode: Mode,
        output: OutputModeRule,
        inputs: &'static [InputContract],
        architectures: &'static [ImplementationSupport],
        scope: SupportScope,
    ) -> Self {
        Self::Supported {
            scope,
            default: ImplementationSupport::cpu(
                operation,
                mode,
                Architecture::Default,
                true,
                &[],
                StrategyGuard::Always,
            ),
            architectures,
            output,
            inputs,
            value_result: None,
        }
    }

    /// Resolves this declaration once, before preparing a serial algorithm.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn select_serial(
        self,
        ctx: &ExecutionContext,
    ) -> Result<StrategySelection, PilError> {
        self.select(
            ctx,
            StrategyInputs {
                width: 0,
                height: 0,
                byte_len: None,
                palette: PaletteKnowledge::Unknown,
                request: RequestFacts::Unspecified,
            },
        )
    }

    /// Assesses known metadata without treating unknown guards as a fallback.
    // VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
    pub fn assess(
        self,
        ctx: &ExecutionContext,
        input: StrategyInputs,
    ) -> Result<SupportAssessment, PilError> {
        let (default, architectures) = match self {
            Self::Rejected { mode, reason, kind } => {
                return Ok(SupportAssessment::Rejected { mode, reason, kind });
            }
            Self::Supported {
                default,
                architectures,
                ..
            } => (default, architectures),
        };
        if ctx.backend() != Backend::Cpu {
            return Err(PilError::InternalError(
                "CPU strategy received a context for another backend".into(),
            ));
        }
        if let Some(guard) = self.assess_inputs(input)? {
            return Ok(SupportAssessment::Deferred {
                implementation: default.id,
                guard,
            });
        }
        for implementation in prioritized_implementations(architectures, ctx.architecture()) {
            if !ctx.permits_architecture(implementation.id.architecture)
                || !ctx.supports_architecture(implementation.id.architecture)
                || !implementation.compiled
                || !implementation
                    .requires
                    .iter()
                    .all(|&arch| ctx.supports_architecture(arch))
            {
                continue;
            }
            match implementation.guard.assess_for_mode(input, default.id.mode) {
                GuardAssessment::Deferred => {
                    return Ok(SupportAssessment::Deferred {
                        implementation: implementation.id,
                        guard: UnresolvedGuard::Implementation(implementation.guard),
                    });
                }
                GuardAssessment::Satisfied => break,
                GuardAssessment::Ineligible => (),
            }
        }
        self.select(ctx, input).map(SupportAssessment::Ready)
    }

    /// Selects only from this declaration using captured capabilities and source facts.
    // VERIFY: source=reviewed@2026-11-13; parity=pass@f32-rank-comparator-cpu-architecture-matrix-20261113; benchmark=pending
    pub(crate) fn select(
        self,
        ctx: &ExecutionContext,
        input: StrategyInputs,
    ) -> Result<StrategySelection, PilError> {
        let (default, architectures) = match self {
            Self::Supported {
                default,
                architectures,
                ..
            } => (default, architectures),
            Self::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        };
        if ctx.backend() != Backend::Cpu {
            return Err(PilError::InternalError(
                "CPU strategy received a context for another backend".into(),
            ));
        }
        let mut selection = StrategySelection {
            requested_backend: ctx.backend(),
            requested_architecture: ctx.architecture(),
            implementation: default.id,
            guard: default.guard,
            fallback: (ctx.architecture() != Architecture::Default)
                .then_some(FallbackReason::MissingSpecialization),
        };
        if ctx.architecture() == Architecture::Default {
            return Ok(selection);
        }
        if !ctx.supports_architecture(ctx.architecture()) {
            selection.fallback = Some(FallbackReason::UnavailableHardware);
        }
        for implementation in prioritized_implementations(architectures, ctx.architecture()) {
            if !ctx.permits_architecture(implementation.id.architecture) {
                continue;
            }
            let reason = if !implementation.compiled {
                FallbackReason::UnavailableBuild
            } else if !ctx.supports_architecture(implementation.id.architecture)
                || !implementation
                    .requires
                    .iter()
                    .all(|&arch| ctx.supports_architecture(arch))
            {
                FallbackReason::UnavailableHardware
            } else {
                match implementation.guard.assess_for_mode(input, default.id.mode) {
                    GuardAssessment::Ineligible => FallbackReason::IneligibleRequest,
                    GuardAssessment::Deferred => {
                        return Err(PilError::InternalError(
                            "strategy preparation requires materialized input facts".into(),
                        ));
                    }
                    GuardAssessment::Satisfied => {
                        selection.implementation = implementation.id;
                        selection.guard = implementation.guard;
                        if implementation.id.architecture == ctx.architecture()
                            && selection.fallback == Some(FallbackReason::MissingSpecialization)
                        {
                            selection.fallback = None;
                        }
                        return Ok(selection);
                    }
                }
            };
            // Record the first failure in preference order. A later, narrower
            // specialization or Default must not erase that explanation.
            if selection.fallback == Some(FallbackReason::MissingSpecialization) {
                selection.fallback = Some(reason);
            }
        }
        Ok(selection)
    }
}
