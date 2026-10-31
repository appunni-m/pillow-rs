//! Typed ImageOps geometry contracts and prepared Resize composition.

// NO_PILLOW_SUPPORT: internal high-depth raster identities compose their
// native Resize strategies, although Pillow has no corresponding mode names.

use crate::compute::{ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    OutputModeRule, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
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
        | Mode::ImageRgba32F => ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            inputs,
            &[],
        ),
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
pub(crate) static FIT_SUPPORT: OperationSupport =
    OperationSupport::composed(
        OperationId::Fit,
        FIT_INPUTS,
        classify_fit,
        BOXED_RESIZE_CHILD,
    );

/// Complete typed support contract for Scale and its nested Resize strategy.
pub(crate) static SCALE_SUPPORT: OperationSupport =
    OperationSupport::composed(
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
                crate::compute::record_pipeline_operation_architecture(
                    resize.selection().implementation.architecture,
                );
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

fn bankers_round(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction == 0.5 {
        if floor % 2.0 == 0.0 { floor } else { floor + 1.0 }
    } else {
        (value + 0.5).floor()
    }
}

fn contain_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 || height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(width) / f64::from(height);
    let size = if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio > target_ratio {
        (
            width,
            bankers_round(f64::from(source_height) / f64::from(source_width)
                * f64::from(width)) as u32,
        )
    } else {
        (
            bankers_round(f64::from(source_width) / f64::from(source_height)
                * f64::from(height)) as u32,
            height,
        )
    };
    validate_contain_cover_size(source, size)?;
    Ok(size)
}

fn cover_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 || height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(width) / f64::from(height);
    let size = if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio < target_ratio {
        if source_width == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        (
            width,
            bankers_round(f64::from(source_height) / f64::from(source_width)
                * f64::from(width)) as u32,
        )
    } else {
        (
            bankers_round(f64::from(source_width) / f64::from(source_height)
                * f64::from(height)) as u32,
            height,
        )
    };
    validate_contain_cover_size(source, size)?;
    Ok(size)
}

fn validate_contain_cover_size(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(), PilError> {
    let empty_width_copy = source.0 == 0 && target.0 == 0 && target.1 == source.1;
    if (target.0 == 0 || target.1 == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

fn fit_box(
    source: (u32, u32),
    target: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
) -> Result<Option<(f64, f64, f64, f64)>, PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    if height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    if source_width == 0 {
        if width == 0 && height == source_height && bleed == 0.0 {
            return Ok(None);
        }
        if width == 0 || height < source_height {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if width == 0 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    let bleed_width = bleed * f64::from(source_width);
    let bleed_height = bleed * f64::from(source_height);
    let live_width = f64::from(source_width) - 2.0 * bleed_width;
    let live_height = f64::from(source_height) - 2.0 * bleed_height;
    let live_ratio = live_width / live_height;
    let target_ratio = f64::from(width) / f64::from(height);
    let (crop_width, crop_height) = if (live_ratio - target_ratio).abs() < 1e-10 {
        (live_width, live_height)
    } else if live_ratio >= target_ratio {
        (target_ratio * live_height, live_height)
    } else {
        (live_width, live_width / target_ratio)
    };
    let crop_left = bleed_width + (live_width - crop_width) * centering.0;
    let crop_top = bleed_height + (live_height - crop_height) * centering.1;
    Ok(Some((
        crop_left,
        crop_top,
        crop_left + crop_width,
        crop_top + crop_height,
    )))
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
        inputs.facts().with_fit(width, height, filter, bleed, centering),
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
    let selection = select_contain(inputs, width, height, filter, context)?;
    let dimensions = contain_dimensions(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
    )?;
    resize_plan(inputs, dimensions.0, dimensions.1, filter, None, selection, context)
}

fn prepare_cover<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<PreparedCover<'a>, PilError> {
    let selection = select_cover(inputs, width, height, filter, context)?;
    let dimensions = cover_dimensions(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
    )?;
    resize_plan(inputs, dimensions.0, dimensions.1, filter, None, selection, context)
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
    let selection = select_fit(inputs, width, height, filter, bleed, centering, context)?;
    let bounds = fit_box(
        (inputs.image.width(), inputs.image.height()),
        (width, height),
        bleed,
        centering,
    )?;
    let Some(bounds) = bounds else {
        return Ok(PreparedGeometry { work: GeometryWork::Copy(inputs.image), selection });
    };
    let effective_filter = if inputs.mode() == Mode::ImagePalette8 {
        ResampleFilter::Nearest
    } else {
        filter
    };
    resize_plan(
        inputs,
        width,
        height,
        effective_filter,
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
    let width = bankers_round(f64::from(inputs.image.width()) * factor) as u32;
    let height = bankers_round(f64::from(inputs.image.height()) * factor) as u32;
    let selection = select_scale(inputs, factor, width, height, filter, context)?;
    resize_plan(inputs, width.max(1), height.max(1), filter, None, selection, context)
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
        PipelineOp::Fit { w, h, filter, bleed, centering } => {
            prepare_fit(inputs, *w, *h, *filter, *bleed, *centering, context)
                .map(crate::compute::registry::PreparedOperation::from_fit)
        }
        PipelineOp::Pad { .. } => {
            super::pad::ops::prepare_pipeline(inputs, op, context)
        }
        PipelineOp::Scale { factor, filter } => prepare_scale(inputs, *factor, *filter, context)
            .map(crate::compute::registry::PreparedOperation::from_scale),
        _ => Err(PilError::ValueError("expected a geometry op".into())),
    }
}
