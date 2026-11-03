//! Wrapper-owned operation routes and their inspectable predicates.

use crate::compute::Backend;
use crate::raster::Mode;

use super::{ModeSupport, OperationId, OutputModeRule, RejectionKind, SupportScope};

/// Result of a wrapper-owned composed route for one exact source mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRouteOutcome {
    /// The declared conversion and child-operation sequence produces its output.
    Produces,
    /// The existing public sequence stops with the declared error.
    Rejects {
        /// Stage where the public route reports the rejection.
        stage: OperationRouteStage,
        /// Existing public rejection message.
        reason: &'static str,
        /// Existing error class.
        kind: RejectionKind,
    },
}

/// Stage in a wrapper-owned composed route where the current behavior stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRouteStage {
    /// Conversion from the source to the intermediate base fails.
    SourceConversion,
    /// Conversion from the intermediate base back to the source mode fails.
    ResultConversion,
    /// The wrapper rejects after constructing the source-to-intermediate route.
    WrapperGuard,
    /// The final child operation rejects the converted inputs.
    ChildAdmission,
}

/// Wrapper-owned execution path declared for one exact operation and mode.
///
/// Multiple paths are listed in dispatch order. The wrapper evaluates the
/// declared guards; the operation-level support view exposes those predicates,
/// fast paths, and child operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRoutePath {
    /// Return a same-mode independent image without extracting a channel.
    IdentityCopy,
    /// Return a copy that also retains palette metadata.
    PalettePreservingCopy,
    /// Return an identity result using the existing immutable pixel buffer.
    SharedBufferIdentity {
        /// Every wrapper predicate that must pass before this path is selected.
        guards: &'static [OperationRouteGuard],
    },
    /// Execute a wrapper-owned eager route through this prepared child.
    EagerCpuChild {
        /// Child operation executed by the eager path.
        operation: OperationId,
        /// Every wrapper predicate that must pass before this path is selected.
        guards: &'static [OperationRouteGuard],
    },
    /// Queue one lazy child operation.
    PipelineChild(OperationId),
    /// Queue one lazy child operation when every declared request guard passes.
    ConditionalPipelineChild {
        /// Child operation selected by this wrapper path.
        operation: OperationId,
        /// Every wrapper predicate that must pass before this path is selected.
        guards: &'static [OperationRouteGuard],
    },
    /// Queue one child operation for every logical band.
    PipelineChildrenPerBand(OperationId),
}

/// Wrapper predicate that gates one [`OperationRoutePath`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationRouteGuard {
    /// The source wrapper is already a concrete loaded image.
    LoadedImage,
    /// The resolved requested backend must be one of these values.
    RequestedBackendIn(&'static [Backend]),
    /// The public input has non-zero width and height.
    NonEmptyInput,
    /// A Draw bitmap request matches one declared raster/context/mask triple.
    DrawBitmapRouteIn(&'static [(Mode, Mode, Mode)]),
    /// The raster's logical pixel count must not exceed this bound.
    PixelCountAtMost(u64),
    /// The raster stores exactly one byte for each logical pixel.
    ByteStorageLengthMatchesPixelCount,
}

/// Request facts used to evaluate wrapper-owned route predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationRouteInputs {
    /// Backend explicitly requested by the caller, if any.
    pub requested_backend: Option<Backend>,
    /// Input width.
    pub width: u32,
    /// Input height.
    pub height: u32,
    /// Whether the wrapper input is already a concrete loaded image.
    pub loaded_image: Option<bool>,
    /// Whether the stored byte length matches the logical pixel count.
    pub byte_storage_length_matches_pixel_count: Option<bool>,
    /// Draw context mode, when the wrapper request has one.
    pub draw_context_mode: Option<Mode>,
    /// Draw bitmap mask mode, when the wrapper request has one.
    pub bitmap_mask_mode: Option<Mode>,
}

impl OperationRouteGuard {
    /// Evaluates this wrapper predicate against facts resolved by its caller.
    #[must_use]
    pub fn matches(self, input_mode: Mode, inputs: OperationRouteInputs) -> bool {
        match self {
            Self::LoadedImage => inputs.loaded_image.unwrap_or(false),
            Self::ByteStorageLengthMatchesPixelCount => inputs
                .byte_storage_length_matches_pixel_count
                .unwrap_or(false),
            Self::RequestedBackendIn(backends) => inputs
                .requested_backend
                .is_some_and(|backend| backends.contains(&backend)),
            Self::NonEmptyInput => inputs.width != 0 && inputs.height != 0,
            Self::DrawBitmapRouteIn(routes) => inputs
                .draw_context_mode
                .zip(inputs.bitmap_mask_mode)
                .is_some_and(|(context, mask)| routes.contains(&(input_mode, context, mask))),
            Self::PixelCountAtMost(maximum) => {
                u64::from(inputs.width) * u64::from(inputs.height) <= maximum
            }
        }
    }
}

impl OperationRoutePath {
    /// Returns the selected child when this conditional pipeline path matches.
    #[must_use]
    pub fn conditional_pipeline_child(
        self,
        input_mode: Mode,
        inputs: OperationRouteInputs,
    ) -> Option<OperationId> {
        let Self::ConditionalPipelineChild { operation, guards } = self else {
            return None;
        };
        guards
            .iter()
            .all(|guard| guard.matches(input_mode, inputs))
            .then_some(operation)
    }
}

/// Complete public route and prepared admission for one exact operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationModeRoute {
    /// The public operation directly queues or prepares its mode strategy.
    Prepared {
        /// Exact prepared strategy or its existing admission rejection.
        support: ModeSupport,
        /// Wrapper intermediate identity if this operation also builds a base.
        intermediate_mode: Option<Mode>,
        /// Additional wrapper paths selected alongside the prepared strategy.
        paths: &'static [OperationRoutePath],
    },
    /// The wrapper owns this route and may select among its declared paths.
    WrapperOwned {
        /// Exact source identity.
        mode: Mode,
        /// Exact mode contract and output identity.
        support: ModeSupport,
        /// Available paths in wrapper dispatch order.
        paths: &'static [OperationRoutePath],
    },
    /// The public wrapper composes child operations for this mode.
    ComposedFallback {
        /// Exact source identity.
        mode: Mode,
        /// Intermediate mode used by the composed route, when it converts.
        intermediate_mode: Option<Mode>,
        /// Pillow-defined route or deliberate core-only extension.
        scope: SupportScope,
        /// Ordered child operations; conversion endpoints use `mode` and
        /// `intermediate_mode` to make their exact mode transitions inspectable.
        operations: &'static [OperationId],
        /// Output identity if the composed route succeeds.
        output: OutputModeRule,
        /// Public result or the exact stage and error where it rejects.
        outcome: OperationRouteOutcome,
        /// Prepared-pipeline admission, which can differ from wrapper support.
        prepared_support: ModeSupport,
    },
}

impl OperationModeRoute {
    /// Returns the wrapper paths declared for this exact source mode.
    #[must_use]
    pub const fn paths(self) -> &'static [OperationRoutePath] {
        match self {
            Self::Prepared { paths, .. } | Self::WrapperOwned { paths, .. } => paths,
            Self::ComposedFallback { .. } => &[],
        }
    }

    /// Returns the admission contract for a directly prepared operation.
    #[must_use]
    pub const fn prepared_support(self) -> ModeSupport {
        match self {
            Self::Prepared { support, .. } => support,
            Self::WrapperOwned { support, .. } => support,
            Self::ComposedFallback { prepared_support, .. } => prepared_support,
        }
    }

    /// Returns the declared intermediate identity used by a wrapper-owned base.
    #[must_use]
    pub const fn intermediate_mode(self) -> Option<Mode> {
        match self {
            Self::Prepared { intermediate_mode, .. }
            | Self::ComposedFallback { intermediate_mode, .. } => intermediate_mode,
            Self::WrapperOwned { .. } => None,
        }
    }

    /// Whether this exact mode has a direct prepared strategy.
    #[must_use]
    pub const fn has_prepared_strategy(self) -> bool {
        matches!(
            self,
            Self::Prepared {
                support: ModeSupport::Supported { .. },
                ..
            }
        )
    }

    /// Whether this wrapper route declares a same-mode or palette copy path.
    #[must_use]
    pub fn has_copy_path(self) -> bool {
        self.paths().iter().any(|path| {
            matches!(
                path,
                OperationRoutePath::IdentityCopy | OperationRoutePath::PalettePreservingCopy
            )
        })
    }

    /// Returns the guards for an eager child path, when the route declares one.
    #[must_use]
    pub fn eager_child_guards(
        self,
        operation: OperationId,
    ) -> Option<&'static [OperationRouteGuard]> {
        self.paths().iter().find_map(|path| match path {
            OperationRoutePath::EagerCpuChild {
                operation: child,
                guards,
            } if *child == operation => Some(*guards),
            _ => None,
        })
    }

    /// Returns the guards for a wrapper-owned shared-buffer identity path.
    #[must_use]
    pub fn shared_buffer_identity_guards(self) -> Option<&'static [OperationRouteGuard]> {
        self.paths().iter().find_map(|path| match path {
            OperationRoutePath::SharedBufferIdentity { guards } => Some(*guards),
            _ => None,
        })
    }

    /// Returns the first declared pipeline child, including eager or
    /// conditional paths, when this route queues one operation.
    #[must_use]
    pub fn pipeline_child(self) -> Option<OperationId> {
        self.paths().iter().find_map(|path| match path {
            OperationRoutePath::PipelineChild(operation)
            | OperationRoutePath::ConditionalPipelineChild { operation, .. } => Some(*operation),
            _ => None,
        })
    }

    /// Returns the declared child repeated once per logical band.
    #[must_use]
    pub fn pipeline_children_per_band(self) -> Option<OperationId> {
        self.paths().iter().find_map(|path| match path {
            OperationRoutePath::PipelineChildrenPerBand(operation) => Some(*operation),
            _ => None,
        })
    }
}
