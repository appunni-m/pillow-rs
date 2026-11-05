//! Wrapper-owned Convert target compositions declared beside typed support.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeTransition, OperationRouteOutcome,
    OperationRouteStage, OperationTargetComposition, OperationTargetCompositionKind,
    OperationTransitionSource, OutputModeRule, RejectionKind, SupportScope,
};
use crate::raster::Mode;

use super::super::ops::classify_mode;

const TO_P: &[OperationModeTransition] = &[OperationModeTransition {
    operation: OperationId::Convert,
    source: OperationTransitionSource::Input,
    target: Mode::ImagePalette8,
}];
const TO_PA: &[OperationModeTransition] = &[OperationModeTransition {
    operation: OperationId::Convert,
    source: OperationTransitionSource::Input,
    target: Mode::ImagePaletteAlpha8,
}];

pub(crate) fn classify_target_composition(
    source: Mode,
    target: Mode,
) -> Option<OperationTargetComposition> {
    if source == Mode::ImageLaPremultiplied8
        && !matches!(target, Mode::ImageLaPremultiplied8 | Mode::ImageLumaA8)
        && prepared_support.scope_for_target(target).is_none()
    {
        return Some(classify_convert_guard(source, target, classify_mode(source)));
    }
    if source != target && matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return classify_palette_composition(source, target, classify_mode(source));
    }
    if is_premultiplied_target(target) {
        return classify_premultiplied_composition(source, target, classify_mode(source));
    }
    None
}

fn classify_palette_composition(
    source: Mode,
    target: Mode,
    prepared_support: ModeSupport,
) -> Option<OperationTargetComposition> {
    let scope = prepared_support.scope_for_target(target)?;
    Some(OperationTargetComposition {
        source_mode: source,
        target_mode: target,
        kind: OperationTargetCompositionKind::ConvertPalette,
        scope,
        transitions: if target == Mode::ImagePalette8 {
            TO_P
        } else {
            TO_PA
        },
        output: OutputModeRule::ByTargetMode,
        outcome: OperationRouteOutcome::Produces,
        prepared_support,
    })
}

fn classify_convert_guard(
    source: Mode,
    target: Mode,
    prepared_support: ModeSupport,
) -> OperationTargetComposition {
    rejected_target_composition(
        source,
        target,
        OperationTargetCompositionKind::ConvertGuard,
        prepared_support.scope().unwrap_or(SupportScope::Pillow),
        prepared_support,
        OperationRouteStage::WrapperGuard,
        "conversion from La to L not supported",
        RejectionKind::ValueError,
    )
}

fn classify_premultiplied_composition(
    source: Mode,
    target: Mode,
    prepared_support: ModeSupport,
) -> Option<OperationTargetComposition> {
    let source_scope = prepared_support.scope().unwrap_or(SupportScope::Pillow);
    if source == Mode::ImagePalette8 {
        return Some(rejected_target_composition(
            source,
            target,
            OperationTargetCompositionKind::ConvertPremultiplied,
            source_scope,
            prepared_support,
            OperationRouteStage::WrapperGuard,
            "conversion not supported",
            RejectionKind::ValueError,
        ));
    }
    if prepared_support.scope_for_target(target).is_some() {
        return None;
    }
    if rejects_luma_to_premultiplied(source) {
        let reason = if target == Mode::ImageLaPremultiplied8 {
            "conversion from L to La not supported"
        } else {
            "conversion from L to RGBa not supported"
        };
        return Some(rejected_target_composition(
            source,
            target,
            OperationTargetCompositionKind::ConvertPremultiplied,
            source_scope,
            prepared_support,
            OperationRouteStage::WrapperGuard,
            reason,
            RejectionKind::ValueError,
        ));
    }
    let (reason, kind) = target_rejection(prepared_support, target)
        .unwrap_or(("image has wrong mode", RejectionKind::ValueError));
    Some(rejected_target_composition(
        source,
        target,
        OperationTargetCompositionKind::ConvertPremultiplied,
        source_scope,
        prepared_support,
        OperationRouteStage::SourceConversion,
        reason,
        kind,
    ))
}

fn rejected_target_composition(
    source: Mode,
    target: Mode,
    kind: OperationTargetCompositionKind,
    scope: SupportScope,
    prepared_support: ModeSupport,
    stage: OperationRouteStage,
    reason: &'static str,
    rejection_kind: RejectionKind,
) -> OperationTargetComposition {
    OperationTargetComposition {
        source_mode: source,
        target_mode: target,
        kind,
        scope,
        transitions: &[],
        output: OutputModeRule::ByTargetMode,
        outcome: OperationRouteOutcome::Rejects {
            stage,
            reason,
            kind: rejection_kind,
        },
        prepared_support,
    }
}

fn target_rejection(support: ModeSupport, target: Mode) -> Option<(&'static str, RejectionKind)> {
    match support {
        ModeSupport::Rejected { reason, kind, .. } => Some((reason, kind)),
        ModeSupport::Supported { inputs, .. } => inputs.iter().find_map(|input| match input {
            InputContract::ConvertTarget { targets, rejection_reason, .. }
                if !targets.contains(&target) =>
            {
                Some((*rejection_reason, RejectionKind::ValueError))
            }
            _ => None,
        }),
    }
}

const fn is_premultiplied_target(mode: Mode) -> bool {
    matches!(mode, Mode::ImageLaPremultiplied8 | Mode::ImageRgbPremultiplied8)
}

const fn rejects_luma_to_premultiplied(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImageLumaA8
            | Mode::ImageI32
            | Mode::ImageF32
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    )
}
