//! Wrapper-owned Convert target compositions declared beside typed support.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeTransition, OperationRouteOutcome,
    OperationRouteStage, OperationTargetComposition, OperationTargetCompositionKind,
    OperationTransitionSource, OutputModeRule, RejectionKind, SupportScope,
};
use crate::raster::Mode;

use super::super::ops::classify_mode;

// NO_PILLOW_SUPPORT: sources outside Pillow's direct RGBX map use Rust composition.
const PILLOW_RGBX_SOURCES: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImagePalette8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageI32,
];

const TO_RGB: &[OperationModeTransition] = &[OperationModeTransition {
    operation: OperationId::Convert,
    source: OperationTransitionSource::Input,
    target: Mode::ImageRgb8,
}];
const TO_RGBA: &[OperationModeTransition] = &[OperationModeTransition {
    operation: OperationId::Convert,
    source: OperationTransitionSource::Input,
    target: Mode::ImageRgba8,
}];
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
const DIRECT_TO_LAB: &[OperationModeTransition] = &[OperationModeTransition {
    operation: OperationId::ConvertLab,
    source: OperationTransitionSource::Input,
    target: Mode::ImageLab8,
}];
const TO_LAB: &[OperationModeTransition] = &[
    OperationModeTransition {
        operation: OperationId::Convert,
        source: OperationTransitionSource::Input,
        target: Mode::ImageRgba8,
    },
    OperationModeTransition {
        operation: OperationId::ConvertLab,
        source: OperationTransitionSource::Previous,
        target: Mode::ImageLab8,
    },
];
const LUMA_TO_LAB: &[OperationModeTransition] = &[
    OperationModeTransition {
        operation: OperationId::Convert,
        source: OperationTransitionSource::Input,
        target: Mode::ImageLuma8,
    },
    OperationModeTransition {
        operation: OperationId::Convert,
        source: OperationTransitionSource::Previous,
        target: Mode::ImageRgba8,
    },
    OperationModeTransition {
        operation: OperationId::ConvertLab,
        source: OperationTransitionSource::Previous,
        target: Mode::ImageLab8,
    },
];
const RGB_TO_LAB: &[OperationModeTransition] = &[
    OperationModeTransition {
        operation: OperationId::Convert,
        source: OperationTransitionSource::Input,
        target: Mode::ImageRgb8,
    },
    OperationModeTransition {
        operation: OperationId::Convert,
        source: OperationTransitionSource::Previous,
        target: Mode::ImageRgba8,
    },
    OperationModeTransition {
        operation: OperationId::ConvertLab,
        source: OperationTransitionSource::Previous,
        target: Mode::ImageLab8,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LabNormalization {
    Direct,
    ToRgba,
    ViaLuma,
    ViaRgb,
}

pub(super) const fn lab_normalization(source: Mode) -> LabNormalization {
    match source {
        Mode::ImageRgb8 | Mode::ImageRgba8 | Mode::ImageRgbx8 => LabNormalization::Direct,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N | Mode::ImageF32 => {
            LabNormalization::ViaLuma
        }
        Mode::ImageYCbCr8 | Mode::ImageHsv8 => LabNormalization::ViaRgb,
        _ => LabNormalization::ToRgba,
    }
}

pub(crate) fn classify_target_composition(
    source: Mode,
    target: Mode,
) -> Option<OperationTargetComposition> {
    if source == Mode::ImageLaPremultiplied8
        && !matches!(target, Mode::ImageLaPremultiplied8 | Mode::ImageLumaA8)
    {
        return Some(classify_convert_guard(source, target, classify_mode(source)));
    }
    if source != target && matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return classify_palette_composition(source, target, classify_mode(source));
    }
    if target == Mode::ImageLab8 && source != Mode::ImageLab8 {
        return classify_lab_composition(source, target, classify_mode(source));
    }
    if is_premultiplied_target(target) {
        return classify_premultiplied_composition(source, target, classify_mode(source));
    }
    if target == Mode::ImageRgbx8 && !uses_prepared_rgbx_route(source) {
        return classify_rgbx_composition(source, target, classify_mode(source));
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

fn classify_lab_composition(
    source: Mode,
    target: Mode,
    prepared_support: ModeSupport,
) -> Option<OperationTargetComposition> {
    let scope = prepared_support.scope_for_target(target)?;
    let transitions = match lab_normalization(source) {
        LabNormalization::Direct => DIRECT_TO_LAB,
        LabNormalization::ToRgba => TO_LAB,
        LabNormalization::ViaLuma => LUMA_TO_LAB,
        LabNormalization::ViaRgb => RGB_TO_LAB,
    };
    Some(OperationTargetComposition {
        source_mode: source,
        target_mode: target,
        kind: OperationTargetCompositionKind::ConvertLab,
        scope,
        transitions,
        output: OutputModeRule::ByTargetMode,
        outcome: OperationRouteOutcome::Produces,
        prepared_support,
    })
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

fn classify_rgbx_composition(
    source: Mode,
    target: Mode,
    prepared_support: ModeSupport,
) -> Option<OperationTargetComposition> {
    // NO_PILLOW_SUPPORT: the internal LA16 route keeps its alpha byte in X,
    // matching LA's native padding-lane conversion.
    let base = if matches!(
        source,
        Mode::ImageLumaA8 | Mode::ImageLumaA16 | Mode::ImagePaletteAlpha8
    ) {
        Mode::ImageRgba8
    } else {
        Mode::ImageRgb8
    };
    let Some(base_scope) = prepared_support.scope_for_target(base) else {
        let (reason, kind) = target_rejection(prepared_support, base)
            .unwrap_or(("image has wrong mode", RejectionKind::ValueError));
        return Some(rejected_target_composition(
            source,
            target,
            OperationTargetCompositionKind::ConvertRgbx,
            prepared_support.scope().unwrap_or(SupportScope::Pillow),
            prepared_support,
            OperationRouteStage::SourceConversion,
            reason,
            kind,
        ));
    };
    let scope = prepared_support.scope_for_target(target).unwrap_or_else(|| {
        if PILLOW_RGBX_SOURCES.contains(&source) {
            base_scope
        } else {
            // NO_PILLOW_SUPPORT: Pillow has no direct conversion from this source to RGBX.
            SupportScope::CoreOnly {
                reason: "Pillow has no direct conversion from this source to RGBX",
            }
        }
    });
    Some(OperationTargetComposition {
        source_mode: source,
        target_mode: target,
        kind: OperationTargetCompositionKind::ConvertRgbx,
        scope,
        transitions: if base == Mode::ImageRgba8 { TO_RGBA } else { TO_RGB },
        output: OutputModeRule::ByTargetMode,
        outcome: OperationRouteOutcome::Produces,
        prepared_support,
    })
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

const fn uses_prepared_rgbx_route(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageRgb8
            | Mode::ImageLumaA8
            | Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageI32
            | Mode::ImageF32
    )
}
