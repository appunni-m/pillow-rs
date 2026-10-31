//! Thumbnail support and prepared mode-aware Reduce/Resize composition.
//!
//! The operation keeps its source mode and passes it unchanged through the
//! typed Reduce and Resize owners. Their mode and architecture declarations
//! select the concrete pixel strategies using the injected context.
//!
//! Mode-specific filter, reducing-gap, and I32/F32 resize decisions are
//! selected through `mode_strategy` before execution.
//! TODO(pillow-rs#3ptb): finish comparing the archived Thumbnail adapter paths
//! with the exact mode-owned Reduce/Resize and fused strategies, then retire
//! its compatibility callers. F32's even 2x2 vector reduction now runs through
//! its nested exact-mode Reduce strategy; general I32 reduction is scalar.
//! TODO(pillow-rs#3ptb): expose the nested Reduce/Resize architecture choices
//! in Thumbnail's own support and selection receipt. Until the composite
//! contract can represent those child choices, its operation receipt stays
//! architecture-unspecified rather than reporting the outer serial declaration.

// NO_PILLOW_SUPPORT: The internal high-depth raster identities use their
// native Reduce and Resize strategies, but Pillow does not expose those modes.

use crate::compute::{Architecture, Backend, ExecutionContext, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OperationInputs, OperationSupport,
    OutputModeRule, StrategyGuard, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

use super::mode_strategy::ThumbnailModeStrategy;

pub(super) const INPUTS: &[crate::compute::registry::support::InputContract] =
    &[crate::compute::registry::support::InputContract::Thumbnail];

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageF32 => super::f32::ops::SUPPORT,
        Mode::ImageLuma8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::serial(OperationId::Thumbnail, mode, INPUTS),
        Mode::ImageI32 => super::i32::ops::SUPPORT,
    }
}

/// Exhaustive typed source-mode support for Thumbnail's prepared strategies.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Thumbnail, INPUTS, classify_mode);

/// Whether a validated RGB PutPixel prefix can use Thumbnail's sparse 2x2 path.
pub(crate) fn rgb_putpixel_fusion_supported(
    image: &DynamicImage,
    writes: &[PipelineOp],
    thumbnail: &PipelineOp,
    mode: Option<Mode>,
    context: &ExecutionContext,
) -> bool {
    if context.backend() != Backend::Cpu
        || context.architecture() != Architecture::Default
        || !matches!(image, DynamicImage::ImageRgb8(_))
    {
        return false;
    }
    if !matches!(mode, None | Some(Mode::ImageRgb8))
        || writes.is_empty()
        || !writes.iter().all(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: false,
                    ..
                }
            )
        })
    {
        return false;
    }
    let PipelineOp::Thumbnail { w, h, filter } = thumbnail else {
        return false;
    };
    let (source_width, source_height) = (image.width(), image.height());
    if source_width < 2
        || source_height < 2
        || source_width % 2 != 0
        || source_height % 2 != 0
        || (source_width as usize).saturating_mul(source_height as usize) < 512 * 512
        || matches!(filter, ResampleFilter::Nearest)
    {
        return false;
    }
    let output_width = (*w).max(1).min(source_width);
    let output_height = (*h).max(1).min(source_height);
    let factor_x = ((f64::from(source_width) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y = ((f64::from(source_height) / f64::from(output_height) / 2.0) as u32).max(1);
    factor_x == 2 && factor_y == 2
}

/// Execute the admitted RGB-only scalar PutPixel/Thumbnail strategy.
pub(crate) fn execute_rgb_putpixel_thumbnail(
    image: &DynamicImage,
    writes: &[PipelineOp],
    thumbnail: &PipelineOp,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    if !rgb_putpixel_fusion_supported(
        image,
        writes,
        thumbnail,
        Some(image.mode()),
        context,
    ) {
        return Err(PilError::InternalError(
            "unsupported RGB PutPixel thumbnail fusion request".into(),
        ));
    }
    super::rgb8::ops::execute_putpixel_thumbnail(image, writes, thumbnail, context)
}

/// Thumbnail plan retaining resolved mode policy, bounds, and backend policy.
pub(crate) struct PreparedThumbnail<'a> {
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    source: (u32, u32),
    output: (u32, u32),
    factors: (u32, u32),
    effective_filter: ResampleFilter,
    reduce: bool,
    use_unboxed_resize: bool,
    stream_i32: bool,
    context: ExecutionContext,
    selection: StrategySelection,
}

struct ThumbnailResizeRequest {
    source: (u32, u32),
    output: (u32, u32),
    factors: (u32, u32),
    filter: ResampleFilter,
    use_unboxed_resize: bool,
}

impl PreparedThumbnail<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let is_composite = !self.stream_i32;
        let result = execute_thumbnail(self);
        if is_composite {
            crate::compute::clear_pipeline_operation_architecture();
        }
        result
    }
}

fn select_mode(
    inputs: OperationInputs<'_>,
    mode: Mode,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match SUPPORT
        .for_mode(mode)
        .assess(context, inputs.facts().with_thumbnail(width, height, filter))?
    {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Thumbnail support remained unresolved after materialization".into(),
        )),
    }
}

/// Prepares a mode-preserving Thumbnail operation and its nested strategies.
pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Thumbnail { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    let mode = inputs.mode();
    let mode_strategy = ThumbnailModeStrategy::for_mode(mode);
    let effective_filter = mode_strategy.effective_filter(*filter);
    let reduce = mode_strategy.should_reduce(effective_filter);
    let source = (inputs.image.width(), inputs.image.height());
    let output = ((*w).max(1).min(source.0), (*h).max(1).min(source.1));
    let factors = crate::compute::helpers::resampling::thumbnail::reduce_factors(
        source,
        output,
        reduce,
    );
    let stream_i32 = *w > 0
        && *h > 0
        && mode_strategy.streams_i32_thumbnail(source, output, effective_filter, reduce);
    let use_unboxed_resize = factors == (1, 1)
        || mode_strategy.uses_unboxed_resize(source, output, factors);
    let selection = select_mode(inputs, mode, *w, *h, *filter, context)?;
    Ok(crate::compute::registry::PreparedOperation::from_thumbnail(
        PreparedThumbnail {
            inputs,
            width: *w,
            height: *h,
            source,
            output,
            factors,
            effective_filter,
            reduce,
            use_unboxed_resize,
            stream_i32,
            context: *context,
            selection,
        },
    ))
}

fn execute_thumbnail(plan: PreparedThumbnail<'_>) -> Result<DynamicImage, PilError> {
    if plan.width == 0 || plan.height == 0 {
        return Err(PilError::ValueError("thumbnail size must be > 0".into()));
    }
    let image = plan.inputs.image;
    let (source_width, source_height) = plan.source;
    if source_width == 0 || source_height == 0 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    let (output_width, output_height) = plan.output;
    let filter = plan.effective_filter;

    if plan.stream_i32 {
        return super::i32::ops::execute(
            image,
            (output_width, output_height),
            filter,
            &plan.context,
            plan.selection.implementation.architecture,
        );
    }

    let (work_image, factors) = if plan.reduce {
        let work_image = reduce_to_thumbnail_work_image(
            plan.inputs,
            plan.factors,
            &plan.context,
        )?;
        (work_image, plan.factors)
    } else {
        (image.clone(), (1, 1))
    };
    resize_thumbnail_work_image(
        &work_image,
        plan.inputs.palette,
        ThumbnailResizeRequest {
            source: (source_width, source_height),
            output: (output_width, output_height),
            factors,
            filter,
            use_unboxed_resize: plan.use_unboxed_resize,
        },
        &plan.context,
    )
}

fn reduce_to_thumbnail_work_image(
    inputs: OperationInputs<'_>,
    factors: (u32, u32),
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    if factors == (1, 1) {
        return Ok(inputs.image.clone());
    }
    let reduced = crate::compute::pool_cpu::ops::reduce::ops::prepare(
        inputs,
        factors,
        context,
    )?
    .execute()?;
    Ok(reduced)
}

fn resize_thumbnail_work_image(
    image: &DynamicImage,
    palette: crate::compute::registry::support::PaletteInput<'_>,
    request: ThumbnailResizeRequest,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    let inputs = OperationInputs { image, palette };
    if request.use_unboxed_resize {
        let op = PipelineOp::Resize {
            w: request.output.0,
            h: request.output.1,
            filter: request.filter,
        };
        return crate::compute::pool_cpu::ops::resize::ops::prepare_pipeline(
            inputs, &op, context,
        )?
        .execute();
    }
    let op = PipelineOp::ResizeBoxed {
        w: request.output.0,
        h: request.output.1,
        filter: request.filter,
        box_coords: (
            0.0,
            0.0,
            f64::from(request.source.0) / f64::from(request.factors.0),
            f64::from(request.source.1) / f64::from(request.factors.1),
        ),
    };
    crate::compute::pool_cpu::ops::resize::ops::prepare_pipeline(inputs, &op, context)?.execute()
}

