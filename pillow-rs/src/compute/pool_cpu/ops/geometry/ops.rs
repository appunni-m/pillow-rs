//! Typed ImageOps geometry contracts and prepared Resize composition.
//! Prepared Resize results, including their physical lanes, flow through Pad's
//! lane composition after each Pad strategy. The direct fused Resize-to-Pad
//! windows belong to L and RGBX, neither of which has omitted physical lanes.
// TODO(pillow-rs#3ptb): verify CPU parity for direct Rust, PyO3, and JavaScript
// geometry calls, including nested Resize identity, validation/error order,
// and architecture selection/fallback receipts. Source call-chain review is
// recorded in `docs/CPU_REFACTOR_INVENTORY.md`.

// NO_PILLOW_SUPPORT: internal high-depth raster identities compose their
// native Resize strategies, although Pillow has no corresponding mode names.

use super::helpers::{contain_dimensions, cover_dimensions, fit_box};
use crate::compute::{
    ExecutionContext,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, OutputModeRule,
        StrategyInputs, StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;
use crate::ops::imageops::{
    normalize_fit_bleed, normalize_fit_centering, scale_dimension_to_u32,
    scale_dimensions_preflight,
};
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

const CONTAIN_INPUTS: &[InputContract] = &[InputContract::ContainParameters];
const COVER_INPUTS: &[InputContract] = &[InputContract::CoverParameters];
const FIT_INPUTS: &[InputContract] = &[InputContract::FitParameters];
const SCALE_INPUTS: &[InputContract] = &[InputContract::ScaleParameters];
const RESIZE_CHILD: &[OperationId] = &[OperationId::Resize];
const BOXED_RESIZE_CHILD: &[OperationId] = &[OperationId::ResizeBoxed];

fn classify_mode(
    operation: OperationId,
    inputs: &'static [InputContract],
    mode: Mode,
) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
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
        | Mode::ImageRgba32F => {
            ModeSupport::cpu(operation, mode, OutputModeRule::Preserve, inputs, &[])
        }
    }
}

fn classify_contain(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Contain, CONTAIN_INPUTS, mode)
}

fn classify_cover(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Cover, COVER_INPUTS, mode)
}

fn classify_fit(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Fit, FIT_INPUTS, mode)
}

fn classify_scale(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Scale, SCALE_INPUTS, mode)
}

/// Complete typed support contract for Contain and its nested Resize strategy.
pub(crate) static CONTAIN_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Contain,
    CONTAIN_INPUTS,
    classify_contain,
    RESIZE_CHILD,
);

/// Complete typed support contract for Cover and its nested Resize strategy.
pub(crate) static COVER_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Cover,
    COVER_INPUTS,
    classify_cover,
    RESIZE_CHILD,
);

/// Complete typed support contract for Fit and its nested boxed Resize strategy.
pub(crate) static FIT_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Fit,
    FIT_INPUTS,
    classify_fit,
    BOXED_RESIZE_CHILD,
);

/// Complete typed support contract for Scale and its nested Resize strategy.
pub(crate) static SCALE_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Scale,
    SCALE_INPUTS,
    classify_scale,
    RESIZE_CHILD,
);

enum GeometryWork<'a> {
    Copy(&'a DynamicImage),
    Resize(crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'a>),
}

/// Prepared result shared by the mode-preserving geometry operations.
pub(crate) struct PreparedGeometry<'a> {
    work: GeometryWork<'a>,
    selection: StrategySelection,
}

pub(crate) type PreparedContain<'a> = PreparedGeometry<'a>;
pub(crate) type PreparedCover<'a> = PreparedGeometry<'a>;
pub(crate) type PreparedFit<'a> = PreparedGeometry<'a>;
pub(crate) type PreparedScale<'a> = PreparedGeometry<'a>;

impl PreparedGeometry<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        match &self.work {
            GeometryWork::Copy(_) => &[],
            GeometryWork::Resize(resize) => std::slice::from_ref(resize.selection()),
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.work {
            GeometryWork::Copy(image) => Ok(image.clone()),
            GeometryWork::Resize(resize) => {
                crate::compute::record_pipeline_operation_selection(resize.selection());
                resize.execute()
            }
        }
    }
}

fn select(
    support: &'static OperationSupport,
    mode: Mode,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "geometry preparation has unresolved input facts".into(),
        )),
    }
}

fn resize_plan<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    selection: StrategySelection,
    context: &ExecutionContext,
) -> Result<PreparedGeometry<'a>, PilError> {
    let resize = crate::compute::pool_cpu::ops::resize::ops::prepare_direct(
        inputs, width, height, filter, bounds, context,
    )?;
    Ok(PreparedGeometry {
        work: GeometryWork::Resize(resize),
        selection,
    })
}

fn select_contain(
    inputs: OperationInputs<'_>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    select(
        &CONTAIN_SUPPORT,
        inputs.mode(),
        inputs.facts().with_contain(width, height, filter),
        context,
    )
}

fn select_cover(
    inputs: OperationInputs<'_>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    select(
        &COVER_SUPPORT,
        inputs.mode(),
        inputs.facts().with_cover(width, height, filter),
        context,
    )
}

fn select_fit(
    inputs: OperationInputs<'_>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    select(
        &FIT_SUPPORT,
        inputs.mode(),
        inputs
            .facts()
            .with_fit(width, height, filter, bleed, centering),
        context,
    )
}

fn select_scale(
    inputs: OperationInputs<'_>,
    factor: f64,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    select(
        &SCALE_SUPPORT,
        inputs.mode(),
        inputs.facts().with_scale(factor, width, height, filter),
        context,
    )
}

fn prepare_contain<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<PreparedContain<'a>, PilError> {
    let dimensions = contain_dimensions(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
    )?;
    // ImageOps.contain computes and validates aspect geometry before
    // delegating to Resize, so direct typed pipeline calls keep that order.
    let selection = select_contain(inputs, width, height, filter, context)?;
    resize_plan(
        inputs,
        dimensions.0,
        dimensions.1,
        filter,
        None,
        selection,
        context,
    )
}

fn prepare_cover<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<PreparedCover<'a>, PilError> {
    let dimensions = cover_dimensions(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
    )?;
    // ImageOps.cover computes and validates aspect geometry before
    // delegating to Resize, so direct typed pipeline calls keep that order.
    let selection = select_cover(inputs, width, height, filter, context)?;
    resize_plan(
        inputs,
        dimensions.0,
        dimensions.1,
        filter,
        None,
        selection,
        context,
    )
}

fn prepare_fit<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    context: &ExecutionContext,
) -> Result<PreparedFit<'a>, PilError> {
    let bleed = normalize_fit_bleed(bleed);
    let centering = (
        normalize_fit_centering(centering.0),
        normalize_fit_centering(centering.1),
    );
    let bounds = fit_box(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
        bleed,
        centering,
    )?;
    // ImageOps.fit computes its crop box before delegating to Resize.
    let selection = select_fit(inputs, width, height, filter, bleed, centering, context)?;
    let Some(bounds) = bounds else {
        return Ok(PreparedGeometry {
            work: GeometryWork::Copy(inputs.image),
            selection,
        });
    };
    resize_plan(
        inputs,
        width,
        height,
        filter,
        Some(bounds),
        selection,
        context,
    )
}

fn prepare_scale<'a>(
    inputs: OperationInputs<'a>,
    factor: f64,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<PreparedScale<'a>, PilError> {
    let source = (inputs.image.width(), inputs.image.height());
    let (rounded_width, rounded_height) = scale_dimensions_preflight(source, factor)?;
    if factor == 1.0 {
        let selection = select_scale(inputs, factor, source.0, source.1, filter, context)?;
        return Ok(PreparedGeometry {
            work: GeometryWork::Copy(inputs.image),
            selection,
        });
    }
    let width = scale_dimension_to_u32(rounded_width)?;
    let height = scale_dimension_to_u32(rounded_height)?;
    if width == 0 || height == 0 {
        if source == (0, 0) {
            let selection = select_scale(inputs, factor, width, height, filter, context)?;
            return Ok(PreparedGeometry {
                work: GeometryWork::Copy(inputs.image),
                selection,
            });
        }
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Validate scale arithmetic and output dimensions before strategy
    // admission, matching the eager ImageOps.scale → Image.resize contract.
    let selection = select_scale(inputs, factor, width, height, filter, context)?;
    resize_plan(inputs, width, height, filter, None, selection, context)
}

/// Prepares Contain, Cover, Fit, or Scale with its exact mode and nested Resize plan.
pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    match op {
        PipelineOp::Contain { w, h, filter } => prepare_contain(inputs, *w, *h, *filter, context)
            .map(crate::compute::registry::PreparedOperation::from_contain),
        PipelineOp::Cover { w, h, filter } => prepare_cover(inputs, *w, *h, *filter, context)
            .map(crate::compute::registry::PreparedOperation::from_cover),
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        } => prepare_fit(inputs, *w, *h, *filter, *bleed, *centering, context)
            .map(crate::compute::registry::PreparedOperation::from_fit),
        PipelineOp::Pad { .. } | PipelineOp::PadTyped { .. } | PipelineOp::PadPositioned { .. } => {
            super::pad::ops::prepare_pipeline(inputs, op, context)
        }
        PipelineOp::Scale { factor, filter } => prepare_scale(inputs, *factor, *filter, context)
            .map(crate::compute::registry::PreparedOperation::from_scale),
        _ => Err(PilError::ValueError("expected a geometry op".into())),
    }
}
