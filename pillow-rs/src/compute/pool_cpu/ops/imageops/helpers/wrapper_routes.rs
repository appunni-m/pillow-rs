//! Wrapper-composition routes shared by ImageOps operations.

use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationModeRoute, OperationRouteOutcome, OperationRouteStage,
    OutputModeRule, RejectionKind, SupportScope,
};
use crate::raster::Mode;

pub(crate) const IMAGEOPS_INVERT_CHILDREN: &[OperationId] = &[OperationId::Eval];
pub(crate) const IMAGEOPS_LUT_CHILDREN: &[OperationId] = &[OperationId::Eval];
pub(crate) const IMAGEOPS_COLORIZE_CHILDREN: &[OperationId] =
    &[OperationId::Convert, OperationId::ImageOpsColorizeLut];
pub(crate) const IMAGEOPS_COLORIZE_LUT_CHILDREN: &[OperationId] = &[OperationId::Eval];

fn scope_for_wrapper_mode(mode: Mode) -> SupportScope {
    if mode.is_pillow_mode() {
        SupportScope::Pillow
    } else {
        SupportScope::CoreOnly {
            reason: "this internal raster identity has no Pillow ImageOps route",
        }
    }
}

fn imageops_lut_mode_support(mode: Mode) -> Option<ModeSupport> {
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return None;
    }
    let support = OperationId::Eval.support().for_mode(mode);
    let scope = match support {
        ModeSupport::Supported { scope, .. } => scope,
        ModeSupport::Rejected { .. } => return Some(support),
    };
    Some(match support {
        ModeSupport::Supported {
            default,
            architectures,
            output,
            inputs,
            value_result,
            ..
        } => ModeSupport::Supported {
            scope,
            default,
            architectures,
            output,
            inputs,
            value_result,
        },
        rejected => rejected,
    })
}

fn imageops_lut_wrapper_rejection(mode: Mode) -> Option<(&'static str, RejectionKind)> {
    match mode {
        Mode::ImagePalette8 => Some((
            "mode P support coming soon",
            RejectionKind::NotImplementedError,
        )),
        _ => None,
    }
}

fn classify_imageops_lut_route(mode: Mode) -> OperationModeRoute {
    let support_scope = scope_for_wrapper_mode(mode);
    let (support, operations, scope, outcome) = match (
        imageops_lut_wrapper_rejection(mode),
        imageops_lut_mode_support(mode),
    ) {
        (Some((reason, kind)), _) => (
            ModeSupport::Rejected { mode, reason, kind },
            &[][..],
            support_scope,
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::WrapperGuard,
                reason,
                kind,
            },
        ),
        (None, Some(support @ ModeSupport::Supported { scope, .. })) => (
            support,
            IMAGEOPS_LUT_CHILDREN,
            scope,
            OperationRouteOutcome::Produces,
        ),
        (None, Some(support @ ModeSupport::Rejected { reason, kind, .. })) => (
            support,
            IMAGEOPS_LUT_CHILDREN,
            support_scope,
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::ChildAdmission,
                reason,
                kind,
            },
        ),
        (None, None) => {
            let reason = crate::compute::pool_cpu::ops::imageops::helpers::strategy::
                unsupported_mode_reason(mode);
            let kind = RejectionKind::OsError;
            (
                ModeSupport::Rejected { mode, reason, kind },
                &[][..],
                support_scope,
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

pub(crate) fn classify_imageops_invert_route(mode: Mode) -> OperationModeRoute {
    let child_support = OperationId::Eval.support().for_mode(mode);
    let wrapper_rejection = imageops_lut_wrapper_rejection(mode).or_else(|| match mode {
        Mode::ImageLumaA8 => Some(("not supported for mode LA", RejectionKind::OsError)),
        Mode::ImagePaletteAlpha8 => Some(("not supported for mode PA", RejectionKind::OsError)),
        Mode::ImageRgba8 => Some(("not supported for mode RGBA", RejectionKind::OsError)),
        Mode::ImageCmyk8 => Some(("not supported for mode CMYK", RejectionKind::OsError)),
        Mode::ImageI32 => Some(("not supported for mode I", RejectionKind::OsError)),
        Mode::ImageF32 => Some(("not supported for mode F", RejectionKind::OsError)),
        Mode::ImageI16 => Some(("not supported for mode I;16", RejectionKind::OsError)),
        Mode::ImageI16L => Some(("not supported for mode I;16L", RejectionKind::OsError)),
        Mode::ImageI16B => Some(("not supported for mode I;16B", RejectionKind::OsError)),
        Mode::ImageI16N => Some(("not supported for mode I;16N", RejectionKind::OsError)),
        _ => None,
    });
    let scope = match (wrapper_rejection, child_support) {
        (Some(_), _) if mode.is_pillow_mode() => SupportScope::Pillow,
        (_, ModeSupport::Supported { scope, .. }) => scope,
        (_, ModeSupport::Rejected { .. }) if mode.is_pillow_mode() => SupportScope::Pillow,
        _ => scope_for_wrapper_mode(mode),
    };
    let (support, operations, outcome) = match wrapper_rejection {
        Some((reason, kind)) => (
            ModeSupport::Rejected { mode, reason, kind },
            &[][..],
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::WrapperGuard,
                reason,
                kind,
            },
        ),
        None => match child_support {
            ModeSupport::Supported { .. } => (
                child_support,
                IMAGEOPS_INVERT_CHILDREN,
                OperationRouteOutcome::Produces,
            ),
            ModeSupport::Rejected { reason, kind, .. } => (
                child_support,
                IMAGEOPS_INVERT_CHILDREN,
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::ChildAdmission,
                    reason,
                    kind,
                },
            ),
        },
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

pub(crate) fn classify_imageops_posterize_route(mode: Mode) -> OperationModeRoute {
    classify_imageops_lut_route(mode)
}

pub(crate) fn classify_imageops_solarize_route(mode: Mode) -> OperationModeRoute {
    classify_imageops_lut_route(mode)
}

pub(crate) fn classify_imageops_colorize_route(mode: Mode) -> OperationModeRoute {
    let scope = scope_for_wrapper_mode(mode);
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

pub(crate) fn classify_imageops_colorize_lut_route(mode: Mode) -> OperationModeRoute {
    let scope = scope_for_wrapper_mode(mode);
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
            let reason = crate::compute::pool_cpu::ops::imageops::helpers::strategy::
                unsupported_mode_reason(mode);
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
