//! Contrast wrapper route and prepared entry points.

use super::ops::{INPUTS, PreparedContrast, classify_contrast_mode, prepare_contrast_mode};
use crate::compute::{
    ExecutionContext,
    registry::support::{
        ModeSupport, OperationId, OperationInputs, OperationModeRoute, OperationRouteOutcome,
        OperationRouteStage, OperationSupport, OutputModeRule, RejectionKind, SupportScope,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

const CONTRAST_CHILDREN: &[OperationId] = &[
    OperationId::Convert,
    OperationId::ConvertLab,
    OperationId::BlendModule,
    OperationId::ExtractBand,
    OperationId::PutAlphaData,
];

const CONTRAST_SOURCE_CONVERSION: &[OperationId] = &[OperationId::Convert];
const CONTRAST_CONVERT_BLEND: &[OperationId] = &[
    OperationId::Convert,
    OperationId::Convert,
    OperationId::BlendModule,
];
const CONTRAST_PA_BLEND: &[OperationId] = &[
    OperationId::Convert,
    OperationId::Convert,
    OperationId::ExtractBand,
    OperationId::PutAlphaData,
    OperationId::BlendModule,
];
const CONTRAST_ROUND_TRIP: &[OperationId] = &[OperationId::Convert, OperationId::Convert];

fn contrast_rejection(
    mode: Mode,
    scope: SupportScope,
    operations: &'static [OperationId],
    stage: OperationRouteStage,
    reason: &'static str,
) -> OperationModeRoute {
    OperationModeRoute::ComposedFallback {
        mode,
        intermediate_mode: Some(Mode::ImageLuma8),
        scope,
        operations,
        output: OutputModeRule::Preserve,
        outcome: OperationRouteOutcome::Rejects {
            stage,
            reason,
            kind: RejectionKind::ValueError,
        },
        prepared_support: classify_contrast_mode(mode),
    }
}

fn classify_contrast_route(mode: Mode) -> OperationModeRoute {
    let prepared_support = classify_contrast_mode(mode);
    if matches!(prepared_support, ModeSupport::Supported { .. }) {
        return OperationModeRoute::Prepared {
            support: prepared_support,
            intermediate_mode: (mode == Mode::ImageLab8).then_some(Mode::ImageLuma8),
            paths: &[],
        };
    }

    match mode {
        Mode::ImageLaPremultiplied8 => contrast_rejection(
            mode,
            SupportScope::Pillow,
            CONTRAST_SOURCE_CONVERSION,
            OperationRouteStage::SourceConversion,
            "conversion from La to L not supported",
        ),
        Mode::ImageRgbPremultiplied8 => contrast_rejection(
            mode,
            SupportScope::Pillow,
            CONTRAST_SOURCE_CONVERSION,
            OperationRouteStage::WrapperGuard,
            "conversion from L to RGBa not supported",
        ),
        Mode::ImagePaletteAlpha8 => contrast_rejection(
            mode,
            SupportScope::Pillow,
            CONTRAST_PA_BLEND,
            OperationRouteStage::ChildAdmission,
            "image has wrong mode",
        ),
        Mode::One
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => contrast_rejection(
            mode,
            SupportScope::Pillow,
            CONTRAST_CONVERT_BLEND,
            OperationRouteStage::ChildAdmission,
            "image has wrong mode",
        ),
        // NO_PILLOW_SUPPORT: this native high-depth conversion-first contract is Rust-only.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => contrast_rejection(
            mode,
            SupportScope::CoreOnly {
                reason: "Contrast's conversion-first route has no Pillow mode identity",
            },
            CONTRAST_ROUND_TRIP,
            OperationRouteStage::ResultConversion,
            "image has wrong mode",
        ),
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageHsv8
        | Mode::ImageLab8 => unreachable!("prepared Contrast mode was already selected"),
    }
}

/// Complete public exact-mode routes and prepared support for Contrast.
/// TODO(pillow-rs#3ptb): verify every route with CPU parity and architecture receipts.
pub(crate) static CONTRAST_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::Contrast,
    INPUTS,
    classify_contrast_route,
    CONTRAST_CHILDREN,
);

fn prepare_contrast<'a>(
    input: OperationInputs<'a>,
    factor: f64,
    context: &ExecutionContext,
) -> Result<PreparedContrast<'a>, PilError> {
    prepare_contrast_mode(
        input.image,
        input.facts().with_contrast(factor),
        factor,
        context,
    )
}

pub(crate) fn prepare_contrast_pipeline<'a>(
    input: OperationInputs<'a>,
    operation: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Contrast { factor } = operation else {
        return Err(PilError::ValueError("expected Contrast op".into()));
    };
    prepare_contrast(input, *factor, context)
        .map(crate::compute::registry::PreparedOperation::from_contrast)
}
