//! Support contracts for the public ImageOps Colorize composition.

use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationModeRoute, OperationRouteOutcome, OperationRouteStage,
    OperationSupport, OutputModeRule, RejectionKind, SupportScope,
};
use crate::raster::Mode;

const IMAGEOPS_COLORIZE_CHILDREN: &[OperationId] =
    &[OperationId::Convert, OperationId::ImageOpsColorizeLut];
const IMAGEOPS_COLORIZE_LUT_CHILDREN: &[OperationId] = &[OperationId::Eval];

fn classify_wrapper_route(mode: Mode) -> OperationModeRoute {
    let scope = if mode.is_pillow_mode() {
        SupportScope::Pillow
    } else {
        SupportScope::CoreOnly {
            reason: "this internal raster identity has no Pillow ImageOps route",
        }
    };
    if mode == Mode::ImageLuma8 {
        return OperationModeRoute::ComposedFallback {
            mode,
            intermediate_mode: Some(Mode::ImageRgb8),
            scope,
            operations: IMAGEOPS_COLORIZE_CHILDREN,
            output: OutputModeRule::Fixed(Mode::ImageRgb8),
            outcome: OperationRouteOutcome::Produces,
            prepared_support: OperationId::Colorize.support().for_mode(mode),
        };
    }

    // Pillow's `assert image.mode == "L"` raises an empty-message assertion.
    let reason = "";
    let kind = RejectionKind::AssertionError;
    OperationModeRoute::ComposedFallback {
        mode,
        intermediate_mode: None,
        scope,
        operations: &[],
        output: OutputModeRule::Fixed(Mode::ImageRgb8),
        outcome: OperationRouteOutcome::Rejects {
            stage: OperationRouteStage::WrapperGuard,
            reason,
            kind,
        },
        prepared_support: ModeSupport::Rejected { mode, reason, kind },
    }
}

/// Full ImageOps Colorize route: L converts to RGB, then applies the public
/// `_lut` helper to the converted wrapper's current mode.
// TODO(pillow-rs#3ptb): Freeze subclass `convert()` results that return P or
// unsupported modes, including the post-conversion error and selection receipt.
pub(crate) static IMAGEOPS_COLORIZE_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsColorize,
    &[],
    classify_wrapper_route,
    IMAGEOPS_COLORIZE_CHILDREN,
);

fn classify_lut_route(mode: Mode) -> OperationModeRoute {
    let scope = if mode.is_pillow_mode() {
        SupportScope::Pillow
    } else {
        SupportScope::CoreOnly {
            reason: "this internal raster identity has no Pillow ImageOps route",
        }
    };
    let (support, operations, outcome) = match mode {
        Mode::ImageLuma8 | Mode::ImageRgb8 => (
            OperationId::Eval.support().for_mode(mode),
            IMAGEOPS_COLORIZE_LUT_CHILDREN,
            OperationRouteOutcome::Produces,
        ),
        Mode::ImagePalette8 => {
            let reason = "mode P support coming soon";
            let kind = RejectionKind::NotImplementedError;
            (
                ModeSupport::Rejected { mode, reason, kind },
                &[][..],
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::WrapperGuard,
                    reason,
                    kind,
                },
            )
        }
        _ => {
            let reason = super::super::helpers::strategy::unsupported_mode_reason(mode);
            let kind = RejectionKind::OsError;
            (
                ModeSupport::Rejected { mode, reason, kind },
                &[][..],
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::WrapperGuard,
                    reason,
                    kind,
                },
            )
        }
    };
    OperationModeRoute::ComposedFallback {
        mode,
        intermediate_mode: None,
        scope,
        operations,
        output: OutputModeRule::Preserve,
        outcome,
        prepared_support: support,
    }
}

/// Support for Colorize's shared `_lut` stage after public Convert returns.
// TODO(pillow-rs#3ptb): Verify mode overrides, output metadata, and selected
// or fallback Eval receipts after public Convert and Point callbacks.
pub(crate) static IMAGEOPS_COLORIZE_LUT_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsColorizeLut,
    &[],
    classify_lut_route,
    IMAGEOPS_COLORIZE_LUT_CHILDREN,
);
