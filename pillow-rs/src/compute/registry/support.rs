//! Typed operation contracts linked to concrete prepared strategies.
//!
//! Family declarations supply exhaustive mode classification. Introspection
//! reads those same declarations; it never infers support from file paths.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

mod request;
use request::GuardAssessment;
pub use request::{
    BandIndexRule, InputContract, ModeChannelCapacity, PaletteKnowledge, PixelWriteRunFacts,
    RequestFacts, SecondaryInputFacts, StrategyGuard, StrategyInputs, UnresolvedGuard,
};

/// Palette dependencies resolved by the Pillow image wrapper before execution.
///
/// `None` means resolution completed and no palette is attached. Lazy support
/// assessment uses [`PaletteKnowledge::Unknown`] until resolution completes.
#[derive(Clone, Copy, Default)]
pub struct PaletteInput<'a> {
    /// Interleaved RGB palette entries, or resolved absence.
    pub rgb: Option<&'a [u8]>,
    /// Palette-entry alpha, separate from a PA image's per-pixel alpha.
    pub alpha: &'a [u8],
}

impl PaletteInput<'_> {
    /// Returns the resolved state without interpreting palette indices.
    #[must_use]
    pub const fn knowledge(self) -> PaletteKnowledge {
        match self.rgb {
            Some(_) => PaletteKnowledge::Present,
            None => PaletteKnowledge::Absent,
        }
    }
}

/// A materialized Pillow raster and its explicitly supplied wrapper dependencies.
///
/// Mode comes solely from the raster variant. This value borrows both inputs;
/// it does not expand palettes, clone pixels, or add a second image mode tag.
#[derive(Clone, Copy)]
pub struct OperationInputs<'a> {
    /// Exact mode-bearing Pillow raster to process.
    pub image: &'a DynamicImage,
    /// Resolved palette metadata, including explicit absence.
    pub palette: PaletteInput<'a>,
}

impl OperationInputs<'_> {
    /// Reads the source identity already carried by the raster variant.
    #[must_use]
    pub const fn mode(self) -> Mode {
        self.image.mode()
    }

    /// Supplies materialized facts to the operation's declared guards.
    #[must_use]
    pub fn facts(self) -> StrategyInputs {
        StrategyInputs {
            width: self.image.width(),
            height: self.image.height(),
            byte_len: Some(self.image.as_bytes().len()),
            palette: self.palette.knowledge(),
            request: RequestFacts::Unspecified,
        }
    }
}

/// Identity of an operation with its own strategy and parameter contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationId {
    /// A crop with coordinates normalized by the image API.
    Crop,
    /// Remove an equal border from all edges, allowing valid empty output.
    CropBorder,
    /// Flip image top-to-bottom while preserving its exact mode.
    Flip,
    /// Mirror image left-to-right while preserving its exact mode.
    Mirror,
    /// Apply one Pillow transpose method while preserving its exact mode.
    Transpose,
    /// Rotate by an arbitrary angle with a typed mode and interpolation strategy.
    Rotate,
    /// Reduce native sample blocks while preserving the declared output mode.
    Reduce,
    /// Pillow's source-mode conversion to an eight-bit luma image.
    Grayscale,
    /// Apply one byte lookup table per native byte band, preserving exact mode.
    Eval,
    /// Invert every byte of an image while preserving its exact mode.
    Invert,
    /// Reduce the stored high bits of native L or RGB samples.
    Posterize,
    /// Invert native L or RGB samples at or above a threshold.
    Solarize,
    /// Map grayscale samples through three color lookup tables.
    Colorize,
    /// Stretch L/RGB contrast from per-channel histogram percentiles.
    Autocontrast,
    /// Equalize L/RGB samples directly and P samples after palette expansion.
    Equalize,
    /// Add a native-mode border while preserving the exact source identity.
    Expand,
    /// Internal band extraction; the public wrapper validates its band selector first.
    ExtractBand,
    /// Apply a 3D color lookup table with a typed target identity.
    Color3DLut,
    /// Relocate native pixels using Pillow's process-global random stream.
    EffectSpread,
    /// Generate Pillow's L-mode Gaussian noise using the process-global stream.
    EffectNoise,
    /// One normalized native-byte pixel write with caller-validated coordinates.
    PutPixel,
    /// An ordered run of adjacent native-byte writes that clones its source once.
    PutPixelBatch,
    /// Replace a normalized byte prefix, retaining the direct payload-layout contract.
    PutData,
    /// Insert or replace a constant alpha sample using the exact source identity.
    PutAlpha,
    /// Replace per-pixel alpha from a mode-bearing secondary image.
    PutAlphaData,
}

impl OperationId {
    /// Returns the contract owned by this operation's execution entry point.
    #[must_use]
    pub fn support(self) -> &'static OperationSupport {
        match self {
            Self::Crop => &crate::compute::pool_cpu::ops::crop::ops::SUPPORT,
            Self::CropBorder => &crate::compute::pool_cpu::ops::imageops::ops::CROP_BORDER_SUPPORT,
            Self::Flip => &crate::compute::pool_cpu::ops::transpose::ops::FLIP_SUPPORT,
            Self::Mirror => &crate::compute::pool_cpu::ops::transpose::ops::MIRROR_SUPPORT,
            Self::Transpose => &crate::compute::pool_cpu::ops::transpose::ops::TRANSPOSE_SUPPORT,
            Self::Rotate => &crate::compute::pool_cpu::ops::rotate::ops::SUPPORT,
            Self::Reduce => &crate::compute::pool_cpu::ops::reduce::ops::SUPPORT,
            Self::Grayscale => &crate::compute::pool_cpu::ops::grayscale::ops::SUPPORT,
            Self::Eval => &crate::compute::pool_cpu::ops::eval::ops::SUPPORT,
            Self::Invert => &crate::compute::pool_cpu::ops::imageops::ops::INVERT_SUPPORT,
            Self::Posterize => &crate::compute::pool_cpu::ops::imageops::ops::POSTERIZE_SUPPORT,
            Self::Solarize => &crate::compute::pool_cpu::ops::imageops::ops::SOLARIZE_SUPPORT,
            Self::Colorize => &crate::compute::pool_cpu::ops::imageops::ops::COLORIZE_SUPPORT,
            Self::Autocontrast => &crate::compute::pool_cpu::ops::imageops::autocontrast::SUPPORT,
            Self::Equalize => &crate::compute::pool_cpu::ops::imageops::equalize::SUPPORT,
            Self::Expand => &crate::compute::pool_cpu::ops::imageops::ops::EXPAND_SUPPORT,
            Self::ExtractBand => &crate::compute::pool_cpu::ops::put::ops::EXTRACT_BAND_SUPPORT,
            Self::Color3DLut => &crate::compute::pool_cpu::ops::color3dlut::ops::SUPPORT,
            Self::EffectSpread => &crate::compute::pool_cpu::ops::effects::ops::EFFECT_SPREAD_SUPPORT,
            Self::EffectNoise => &crate::compute::pool_cpu::ops::effects::ops::EFFECT_NOISE_SUPPORT,
            Self::PutPixel => &crate::compute::pool_cpu::ops::put::ops::PUT_PIXEL_SUPPORT,
            Self::PutPixelBatch => &crate::compute::pool_cpu::ops::put::ops::PUT_PIXEL_BATCH_SUPPORT,
            Self::PutData => &crate::compute::pool_cpu::ops::put::ops::PUT_DATA_SUPPORT,
            Self::PutAlpha => &crate::compute::pool_cpu::ops::put::ops::PUT_ALPHA_SUPPORT,
            Self::PutAlphaData => &crate::compute::pool_cpu::ops::put::ops::PUT_ALPHA_DATA_SUPPORT,
        }
    }
}

/// Relationship between an operation's input and output identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputModeRule {
    /// Preserve the exact input identity, including palette and byte order.
    Preserve,
    /// Construct this exact output identity independently of the source mode.
    Fixed(Mode),
    /// Exact output identities for each admitted payload mode of a byte-prefix write.
    ByPayload(&'static [PayloadOutput]),
    /// Output identity supplied as a typed target-mode operation parameter.
    ByTargetMode,
}

/// An operation-owned relation between a typed payload and its output identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadOutput {
    /// Canonical mode whose sample bytes the caller normalized.
    pub payload: Mode,
    /// Exact output variant selected for this operation's source identity.
    pub output: Mode,
}

/// Existing error class for a mode rejected by an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionKind {
    /// Pillow's invalid-value or unsupported conversion error.
    ValueError,
    /// Pillow rejects this mode with an operating-system error.
    OsError,
    /// The existing native implementation does not implement this storage type.
    NotImplementedError,
    /// The existing native implementation rejects the carrier's byte shape.
    InternalError,
}

impl RejectionKind {
    pub(crate) fn into_error(self, reason: &'static str) -> PilError {
        match self {
            Self::ValueError => PilError::ValueError(reason.into()),
            Self::OsError => PilError::OsError(reason.into()),
            Self::NotImplementedError => PilError::NotImplementedError(reason.into()),
            Self::InternalError => PilError::InternalError(reason.into()),
        }
    }
}

/// Stable identity of an executable implementation, independent of its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplementationId {
    /// Owning operation.
    pub operation: OperationId,
    /// Exact raster semantics implemented.
    pub mode: Mode,
    /// Executor owning this implementation.
    pub backend: Backend,
    /// Concrete architecture implementation.
    pub architecture: Architecture,
}

/// Build availability and exact instruction requirement of an implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementationSupport {
    /// Identity also recorded on the executable plan.
    pub id: ImplementationId,
    /// Whether this implementation exists in the current build.
    pub compiled: bool,
    /// Instructions required before the implementation may execute.
    pub requires: &'static [Architecture],
    /// Request condition consumed by selection, not a separate support predicate.
    pub guard: StrategyGuard,
}

impl ImplementationSupport {
    /// Binds a CPU implementation identity to its precise build and request contract.
    pub const fn cpu(
        operation: OperationId,
        mode: Mode,
        architecture: Architecture,
        compiled: bool,
        requires: &'static [Architecture],
        guard: StrategyGuard,
    ) -> Self {
        Self {
            id: ImplementationId { operation, mode, backend: Backend::Cpu, architecture },
            compiled,
            requires,
            guard,
        }
    }
}

/// Whether a supported mode follows Pillow's operation contract or extends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportScope {
    /// This exact operation/mode route is defined by Pillow.
    Pillow,
    /// This route is a native core extension beyond Pillow's operation surface.
    CoreOnly {
        /// Why the extra route has no Pillow parity contract.
        reason: &'static str,
    },
}

/// Why execution selected a declared fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// This operation and mode have no specialization for the requested architecture.
    MissingSpecialization,
    /// A declared specialization was excluded by the compilation target or features.
    UnavailableBuild,
    /// Captured host capabilities do not satisfy the requested implementation.
    UnavailableHardware,
    /// A declared specialization cannot handle this request's shape or layout.
    IneligibleRequest,
}

/// The actual choice retained by a prepared operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategySelection {
    /// The backend supplied by the router.
    pub requested_backend: Backend,
    /// The architecture policy supplied by the router.
    pub requested_architecture: Architecture,
    /// The concrete implementation that will execute.
    pub implementation: ImplementationId,
    /// Why a preferred declaration could not execute, including a narrower
    /// specialization or Default selected after that failure.
    pub fallback: Option<FallbackReason>,
}

/// Contextual admission result without executing or inventing lazy input facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportAssessment {
    /// All required facts are known for this strategy selection.
    Ready(StrategySelection),
    /// This source identity has an existing semantic rejection.
    Rejected {
        /// Exact rejected source mode.
        mode: Mode,
        /// Existing public rejection message.
        reason: &'static str,
        /// Existing error class, shared with preparation and execution admission.
        kind: RejectionKind,
    },
    /// A preferred strategy requires input facts not yet materialized.
    Deferred {
        /// Concrete declaration whose eligibility remains unresolved.
        implementation: ImplementationId,
        /// Guard to resolve at preparation, using the same declaration.
        guard: UnresolvedGuard,
    },
}

/// Explicit supported or rejected outcome for one exact raster mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSupport {
    /// Concrete implementations for this exact source identity.
    Supported {
        /// Whether the declared route is Pillow-defined or a core extension.
        scope: SupportScope,
        /// Default strategy bound by the mode declaration.
        default: ImplementationSupport,
        /// Concrete specializations in the operation's declared preference order.
        architectures: &'static [ImplementationSupport],
        /// Output identity rule.
        output: OutputModeRule,
        /// Additional requirements imposed by this mode's implementation.
        inputs: &'static [InputContract],
    },
    /// An existing semantic rejection rather than an implementation fallback.
    Rejected {
        /// Rejected raster identity.
        mode: Mode,
        /// Existing public rejection message.
        reason: &'static str,
        /// Existing mode rejection class, rather than a uniform ValueError replacement.
        kind: RejectionKind,
    },
}

impl ModeSupport {
    /// Returns whether this mode route is Pillow-defined or a core extension.
    #[must_use]
    pub const fn scope(self) -> Option<SupportScope> {
        match self {
            Self::Supported { scope, .. } => Some(scope),
            Self::Rejected { .. } => None,
        }
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
        let scope = match mode.pillow_name() {
            Some(_) => SupportScope::Pillow,
            None => SupportScope::CoreOnly {
                reason: "this internal raster identity has no Pillow mode spelling",
            },
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
            default: ImplementationSupport::cpu(operation, mode, Architecture::Default,
                true, &[], StrategyGuard::Always),
            architectures,
            output,
            inputs,
        }
    }

    /// Resolves this declaration once, before preparing a serial algorithm.
    pub(crate) fn select_serial(
        self,
        ctx: &ExecutionContext,
    ) -> Result<StrategySelection, PilError> {
        self.select(ctx, StrategyInputs { width: 0, height: 0, byte_len: None,
            palette: PaletteKnowledge::Unknown, request: RequestFacts::Unspecified })
    }

    /// Assesses known metadata without treating unknown guards as a fallback.
    pub fn assess(
        self,
        ctx: &ExecutionContext,
        input: StrategyInputs,
    ) -> Result<SupportAssessment, PilError> {
        let (default, architectures, inputs) = match self {
            Self::Rejected { mode, reason, kind } =>
                return Ok(SupportAssessment::Rejected { mode, reason, kind }),
            Self::Supported { default, architectures, inputs, .. } => (default, architectures, inputs),
        };
        if ctx.backend() != Backend::Cpu {
            return Err(PilError::InternalError("CPU strategy received a context for another backend".into()));
        }
        let mut unresolved = None;
        for contract in inputs {
            if let Some(guard) = contract.assess(input)? {
                if unresolved.is_none() {
                    unresolved = Some(guard);
                }
            }
        }
        if let Some(guard) = unresolved {
            return Ok(SupportAssessment::Deferred { implementation: default.id, guard });
        }
        if ctx.supports_architecture(ctx.architecture()) {
            for implementation in architectures {
                if !ctx.permits_architecture(implementation.id.architecture)
                    || !implementation.compiled
                    || !implementation.requires.iter().all(|&arch| ctx.supports_architecture(arch)) {
                    continue;
                }
                match implementation.guard.assess(input) {
                    GuardAssessment::Deferred => return Ok(SupportAssessment::Deferred {
                        implementation: implementation.id,
                        guard: UnresolvedGuard::Implementation(implementation.guard),
                    }),
                    GuardAssessment::Satisfied => break,
                    GuardAssessment::Ineligible => (),
                }
            }
        }
        self.select(ctx, input).map(SupportAssessment::Ready)
    }

    /// Selects only from this declaration using captured capabilities and source facts.
    pub(crate) fn select(
        self,
        ctx: &ExecutionContext,
        input: StrategyInputs,
    ) -> Result<StrategySelection, PilError> {
        let (default, architectures) = match self {
            Self::Supported { default, architectures, .. } => (default, architectures),
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
            fallback: (ctx.architecture() != Architecture::Default)
                .then_some(FallbackReason::MissingSpecialization),
        };
        if ctx.architecture() == Architecture::Default {
            return Ok(selection);
        }
        if !ctx.supports_architecture(ctx.architecture()) {
            selection.fallback = Some(FallbackReason::UnavailableHardware);
            return Ok(selection);
        }
        for implementation in architectures {
            if !ctx.permits_architecture(implementation.id.architecture) {
                continue;
            }
            let reason = if !implementation.compiled {
                FallbackReason::UnavailableBuild
            } else if !implementation.requires.iter().all(|&arch| ctx.supports_architecture(arch)) {
                FallbackReason::UnavailableHardware
            } else {
                match implementation.guard.assess(input) {
                    GuardAssessment::Ineligible => FallbackReason::IneligibleRequest,
                    GuardAssessment::Deferred => return Err(PilError::InternalError(
                        "strategy preparation requires materialized input facts".into(),
                    )),
                    GuardAssessment::Satisfied => {
                        selection.implementation = implementation.id;
                        if selection.fallback == Some(FallbackReason::MissingSpecialization) {
                            selection.fallback = None;
                        }
                        return Ok(selection);
                    },
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

/// Inspectable support owned by one operation, beside its execution entry point.
pub struct OperationSupport {
    /// Exact operation identity.
    pub operation: OperationId,
    /// Requirements checked during assessment and preparation.
    pub inputs: &'static [InputContract],
    classify: fn(Mode) -> ModeSupport,
}

impl OperationSupport {
    /// Binds an operation to the classifier generated with its concrete strategies.
    pub const fn new(
        operation: OperationId,
        inputs: &'static [InputContract],
        classify: fn(Mode) -> ModeSupport,
    ) -> Self {
        Self { operation, inputs, classify }
    }

    /// Returns this operation's explicit contract for one mode.
    #[must_use]
    pub fn for_mode(&self, mode: Mode) -> ModeSupport {
        (self.classify)(mode)
    }

    /// Enumerates all identities, including rejected and internal modes.
    pub fn modes(&self) -> impl Iterator<Item = ModeSupport> {
        Mode::ALL.into_iter().map(self.classify)
    }
}
