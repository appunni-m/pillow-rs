//! Typed operation contracts linked to concrete prepared strategies.
//!
//! Family declarations supply exhaustive mode classification. Introspection
//! reads those same declarations; it never infers support from file paths.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

mod mode;
mod operation;
mod request;
mod route;
mod target;

pub use operation::OperationId;
use request::GuardAssessment;
pub use request::{
    BandIndexRule, DrawRequestKind, DrawTextMaskKind, ExpandFillRule, F32RankCompatibility,
    InputContract, MergeBandModeRule, ModeChannelCapacity, PadFillRule, PaletteKnowledge,
    PasteInputRules, PasteMaskRule, PixelWriteRunFacts, RequestFacts, SecondaryInputFacts,
    StrategyGuard, StrategyInputs, UnresolvedGuard,
};
pub use route::{
    OperationModeRoute, OperationRouteGuard, OperationRouteInputs, OperationRouteOutcome,
    OperationRoutePath, OperationRouteStage,
};
pub use target::{
    OperationModeTransition, OperationTargetComposition, OperationTargetCompositionKind,
    OperationTransitionSource,
};

/// Exact mask identities accepted by image-backed alpha replacement.
pub(crate) const PUT_ALPHA_DATA_MASK_MODES: &[Mode] = &[Mode::One, Mode::ImageLuma8];

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

/// Relationship between an operation's input and output identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputModeRule {
    /// The operation returns a non-raster result.
    NoRasterResult,
    /// Preserve the exact input identity, including palette and byte order.
    Preserve,
    /// Construct this exact output identity independently of the source mode.
    Fixed(Mode),
    /// Exact output identities for each admitted payload mode of a byte-prefix write.
    ByPayload(&'static [PayloadOutput]),
    /// Output identity supplied as a typed target-mode operation parameter.
    ByTargetMode,
    /// Target-mode output whose palette bytes are returned alongside the raster.
    ByTargetModeWithPalette,
}

/// Shape of a non-raster value returned by an operation for one exact mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationValueResult {
    /// Result shape of one exact-mode pixel read.
    Pixel(PixelValueShape),
    /// Histogram output concatenates fixed-size bins in band order.
    Histogram {
        /// Number of output bands, which may preserve a compatibility band
        /// synthesized by an earlier projection.
        result_bands: usize,
        /// Number of bins in each output band.
        bins_per_band: usize,
    },
}

/// Native value shape returned by a direct pixel-read operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelValueShape {
    /// One unsigned 8-bit sample.
    U8Scalar,
    /// One signed 32-bit sample.
    I32Scalar,
    /// One f64 API value sourced from an F32 sample.
    F64Scalar,
    /// A tuple of byte samples in Pillow band order.
    U8Components {
        /// Number of byte samples in the returned tuple.
        bands: u8,
    },
    /// A tuple of native unsigned 16-bit samples.
    U16Components {
        /// Number of native unsigned 16-bit samples in the tuple.
        bands: u8,
    },
    /// A tuple of native 32-bit floating-point samples.
    F32Components {
        /// Number of float32 samples in the tuple.
        bands: u8,
    },
    /// The Rust/JavaScript convenience API's fixed four-byte RGBA result.
    Rgba8,
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
    /// Pillow rejects the operation through an assertion contract.
    AssertionError,
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
            Self::AssertionError => PilError::AssertionError(reason.into()),
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
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub const fn cpu(
        operation: OperationId,
        mode: Mode,
        architecture: Architecture,
        compiled: bool,
        requires: &'static [Architecture],
        guard: StrategyGuard,
    ) -> Self {
        Self {
            id: ImplementationId {
                operation,
                mode,
                backend: Backend::Cpu,
                architecture,
            },
            compiled,
            requires,
            guard,
        }
    }
}

/// Prefer the context's exact architecture, then retain declaration order for
/// specialized siblings and permitted lower-architecture fallbacks.
// TODO(pillow-rs#3ptb): verify exact/lower architecture selection and fallback
// reasons across each operation's declared strategy set in the frozen parity.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn prioritized_implementations<'a>(
    architectures: &'a [ImplementationSupport],
    requested: Architecture,
) -> impl Iterator<Item = &'a ImplementationSupport> {
    architectures
        .iter()
        .filter(move |implementation| implementation.id.architecture == requested)
        .chain(
            architectures
                .iter()
                .filter(move |implementation| implementation.id.architecture != requested),
        )
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

impl FallbackReason {
    /// Returns a stable diagnostic name for this strategy fallback.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MissingSpecialization => "missing_specialization",
            Self::UnavailableBuild => "unavailable_build",
            Self::UnavailableHardware => "unavailable_hardware",
            Self::IneligibleRequest => "ineligible_request",
        }
    }
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
    /// The exact request guard selected for this implementation, including a
    /// mode-local specialization when multiple strategies share one architecture.
    pub guard: StrategyGuard,
    /// Why a preferred declaration could not execute, including a narrower
    /// specialization or Default selected after that failure.
    pub fallback: Option<FallbackReason>,
}

/// Contextual admission result without executing or inventing lazy input facts.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        /// Shape of the non-raster value returned for this exact mode, if any.
        value_result: Option<OperationValueResult>,
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

/// Inspectable support owned by one operation, beside its execution entry point.
pub struct OperationSupport {
    /// Exact operation identity.
    pub operation: OperationId,
    /// Requirements checked during assessment and preparation.
    pub inputs: &'static [InputContract],
    /// Child operation contracts referenced by composed routes or strategies.
    composed_operations: &'static [OperationId],
    classify: Option<fn(Mode) -> ModeSupport>,
    classify_route: Option<fn(Mode) -> OperationModeRoute>,
    classify_target_composition: Option<fn(Mode, Mode) -> Option<OperationTargetComposition>>,
    /// Output identity retained by lazy metadata when a source mode is rejected.
    rejected_output: Option<OutputModeRule>,
}

impl OperationSupport {
    /// Binds an operation to the classifier generated with its concrete strategies.
    pub const fn new(
        operation: OperationId,
        inputs: &'static [InputContract],
        classify: fn(Mode) -> ModeSupport,
    ) -> Self {
        Self {
            operation,
            inputs,
            composed_operations: &[],
            classify: Some(classify),
            classify_route: None,
            classify_target_composition: None,
            rejected_output: None,
        }
    }

    /// Declares nested prepared operations that contribute concrete strategies.
    pub const fn composed(
        operation: OperationId,
        inputs: &'static [InputContract],
        classify: fn(Mode) -> ModeSupport,
        composed_operations: &'static [OperationId],
    ) -> Self {
        Self {
            operation,
            inputs,
            composed_operations,
            classify: Some(classify),
            classify_route: None,
            classify_target_composition: None,
            rejected_output: None,
        }
    }

    /// Binds complete wrapper routes and their direct prepared admissions.
    pub const fn routed(
        operation: OperationId,
        inputs: &'static [InputContract],
        classify_route: fn(Mode) -> OperationModeRoute,
        composed_operations: &'static [OperationId],
    ) -> Self {
        Self {
            operation,
            inputs,
            composed_operations,
            classify: None,
            classify_route: Some(classify_route),
            classify_target_composition: None,
            rejected_output: None,
        }
    }

    /// Declares the output identity exposed by lazy metadata before a rejected
    /// source mode reports its operation error.
    #[must_use]
    pub const fn with_rejected_output(mut self, output: OutputModeRule) -> Self {
        self.rejected_output = Some(output);
        self
    }

    /// Returns the declared wrapper paths for one exact source mode.
    #[must_use]
    pub fn route_paths_for_mode(&self, mode: Mode) -> &'static [OperationRoutePath] {
        self.route_for_mode(mode).paths()
    }

    /// Selects the first conditional pipeline child whose wrapper guards pass.
    #[must_use]
    pub fn conditional_pipeline_child(
        &self,
        mode: Mode,
        inputs: OperationRouteInputs,
    ) -> Option<OperationId> {
        self.route_paths_for_mode(mode)
            .iter()
            .find_map(|path| path.conditional_pipeline_child(mode, inputs))
    }

    /// Adds exact source/target wrapper compositions to the operation view.
    ///
    /// Direct target strategies remain in each source mode's `ModeSupport`;
    /// this classifier declares only the wrapper-owned pair routes that add
    /// child transitions or wrapper-specific outcomes.
    #[must_use]
    pub const fn with_target_compositions(
        mut self,
        classify: fn(Mode, Mode) -> Option<OperationTargetComposition>,
    ) -> Self {
        self.classify_target_composition = Some(classify);
        self
    }

    /// Returns the wrapper-owned route for an exact source/target pair, if any.
    #[must_use]
    pub fn target_composition_for(
        &self,
        source: Mode,
        target: Mode,
    ) -> Option<OperationTargetComposition> {
        self.classify_target_composition
            .and_then(|classify| classify(source, target))
    }

    /// Enumerates wrapper-owned target compositions for one exact source mode.
    pub fn target_compositions_for(
        &self,
        source: Mode,
    ) -> impl Iterator<Item = OperationTargetComposition> + '_ {
        Mode::ALL.into_iter().filter_map(move |target| {
            self.classify_target_composition
                .and_then(|classify| classify(source, target))
        })
    }

    /// Returns the declared lazy output identity for rejected source modes.
    #[must_use]
    pub const fn rejected_output(&self) -> Option<OutputModeRule> {
        self.rejected_output
    }

    /// Returns the child operation contracts referenced by this declaration.
    #[must_use]
    pub const fn composed_operations(&self) -> &'static [OperationId] {
        self.composed_operations
    }

    /// Returns the child operation contracts linked to this declaration.
    #[must_use]
    pub fn composed_supports(&self) -> impl Iterator<Item = &'static OperationSupport> + '_ {
        self.composed_operations
            .iter()
            .copied()
            .map(OperationId::support)
    }

    /// Returns this operation's explicit contract for one mode.
    #[must_use]
    pub fn for_mode(&self, mode: Mode) -> ModeSupport {
        match self.classify_route {
            Some(classify_route) => classify_route(mode).prepared_support(),
            None => self
                .classify
                .expect("operation support needs a mode classifier")(mode),
        }
    }

    /// Returns the full public route, including wrapper fallbacks and their outcomes.
    #[must_use]
    pub fn route_for_mode(&self, mode: Mode) -> OperationModeRoute {
        match self.classify_route {
            Some(classify_route) => classify_route(mode),
            None => OperationModeRoute::Prepared {
                support: self
                    .classify
                    .expect("operation support needs a mode classifier")(
                    mode
                ),
                intermediate_mode: None,
                paths: &[],
            },
        }
    }

    /// Returns the contract scope for one exact source and target mode pair.
    #[must_use]
    pub fn scope_for_target(&self, source: Mode, target: Mode) -> Option<SupportScope> {
        if let Some(composition) = self.target_composition_for(source, target) {
            return match composition.outcome {
                OperationRouteOutcome::Produces => Some(composition.scope),
                OperationRouteOutcome::Rejects { .. } => None,
            };
        }
        self.for_mode(source).scope_for_target(target)
    }

    /// Enumerates all identities, including rejected and internal modes.
    pub fn modes(&self) -> impl Iterator<Item = ModeSupport> {
        Mode::ALL.into_iter().map(|mode| self.for_mode(mode))
    }

    /// Enumerates all exact modes with their complete public routes.
    pub fn routes(&self) -> impl Iterator<Item = OperationModeRoute> + '_ {
        Mode::ALL.into_iter().map(|mode| self.route_for_mode(mode))
    }
}
