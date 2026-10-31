//! AlphaComposite support contract and exact-mode strategy orchestration.
//!
//! The preserved serial entry point ignores `dest` and `src`; retain that
//! behavior until the frozen CPU cases establish the offset contract.

use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationInputs, OperationSupport, RejectionKind,
    StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) enum PreparedAlphaComposite<'a> {
    ImageLumaA8(super::luma_a8::ops::Strategy<'a>),
    ImageRgba8(super::rgba8::ops::Strategy<'a>),
}

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Complete support contract for Pillow's LA and RGBA AlphaComposite modes.
pub(crate) static ALPHA_COMPOSITE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::AlphaComposite, &[], classify_mode);

impl PreparedAlphaComposite<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::ImageLumaA8(strategy) => strategy.selection(),
            Self::ImageRgba8(strategy) => strategy.selection(),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::ImageLumaA8(strategy) => strategy.execute(),
            Self::ImageRgba8(strategy) => strategy.execute(),
        }
    }
}

pub(crate) fn assess(
    inputs: OperationInputs<'_>,
    source: crate::compute::registry::support::SecondaryInputFacts,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let facts = inputs.facts().with_same_mode_secondary_image(source);
    classify_mode(inputs.mode()).assess(ctx, facts)
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    source: Arc<DynamicImage>,
    ctx: &ExecutionContext,
) -> Result<PreparedAlphaComposite<'a>, PilError> {
    let secondary = crate::compute::registry::support::SecondaryInputFacts {
        mode: source.mode(),
        width: source.width(),
        height: source.height(),
        byte_len: Some(source.as_bytes().len()),
    };
    let selection = match assess(input, secondary, ctx)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "AlphaComposite preparation received unresolved image facts".into(),
            ));
        }
    };

    match input.image {
        DynamicImage::ImageLumaA8(destination) => {
            Ok(PreparedAlphaComposite::ImageLumaA8(super::luma_a8::ops::prepare(
                destination,
                source,
                selection,
            )?))
        }
        DynamicImage::ImageRgba8(destination) => {
            Ok(PreparedAlphaComposite::ImageRgba8(super::rgba8::ops::prepare(
                destination,
                source,
                selection,
            )?))
        }
        _ => Err(PilError::InternalError(
            "AlphaComposite support and mode routing disagree".into(),
        )),
    }
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::AlphaComposite { source, .. } = op else {
        return Err(PilError::ValueError("expected AlphaComposite op".into()));
    };
    if let ModeSupport::Rejected { reason, kind, .. } = classify_mode(input.mode()) {
        return Err(kind.into_error(reason));
    }
    prepare(input, source.materialized_shared()?, ctx)
        .map(crate::compute::registry::PreparedOperation::from_alpha_composite)
}

/// Prepares the full-frame RGBA fusion using each operation's declared strategy.
pub(crate) fn prepare_mirror_fusion<'a>(
    image: &'a DynamicImage,
    alpha_composite: &PipelineOp,
    mirror: &PipelineOp,
    ctx: &ExecutionContext,
) -> Result<Option<super::rgba8::ops::MirrorFusionStrategy<'a>>, PilError> {
    let PipelineOp::AlphaComposite {
        source,
        dest: (0, 0),
        src: (0, 0),
    } = alpha_composite
    else {
        return Ok(None);
    };
    if !matches!(mirror, PipelineOp::Mirror) {
        return Ok(None);
    }
    let DynamicImage::ImageRgba8(destination) = image else {
        return Ok(None);
    };

    let source = source.materialized_shared()?;
    let DynamicImage::ImageRgba8(_) = source.as_ref() else {
        return Ok(None);
    };
    let prepared = prepare(
        OperationInputs {
            image,
            palette: crate::compute::registry::support::PaletteInput::default(),
        },
        Arc::clone(&source),
        ctx,
    )?;
    let PreparedAlphaComposite::ImageRgba8(prepared) = prepared else {
        return Ok(None);
    };
    let selection = *prepared.selection();
    super::rgba8::ops::prepare_mirror_fusion(destination, source, selection, ctx)
}
