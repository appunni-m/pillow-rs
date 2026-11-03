//! Thumbnail support and prepared mode-aware Reduce/Resize composition.
//!
//! The operation keeps its source mode and passes it unchanged through the
//! typed Reduce and Resize owners. Their mode and architecture declarations
//! select the concrete pixel strategies using the injected context.
//!
//! Mode-specific filter, reducing-gap, and I32/F32 resize decisions are
//! selected through `mode_strategy` before execution.
//! The archived mode branches are source-reviewed against these owners: `1`
//! `1`, `P`, and `PA` force nearest; straight-alpha `LA` and `RGBA` skip
//! reducing-gap; premultiplied `La` reduces its stored samples like the
//! archived implementation. F32 and I32 use exact typed owners; all other
//! modes compose native Reduce and Resize. No active caller uses the archived
//! adapter.
//! TODO(pillow-rs#3ptb): complete Thumbnail parity for every declared exact
//! mode and architecture in the operation × mode × architecture matrix.
//! F32's even 2x2 vector reduction runs through its nested exact-mode Reduce
//! strategy; general I32 reduction is scalar.
//! Thumbnail's support contract links its conditional Reduce and Resize
//! children; the prepared receipt records the concrete children selected for
//! the exact mode, request, and injected architecture context.

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
const COMPOSED_OPERATIONS: &[OperationId] = &[
    OperationId::Reduce,
    OperationId::Resize,
    OperationId::ResizeBoxed,
];

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
    OperationSupport::composed(
        OperationId::Thumbnail,
        INPUTS,
        classify_mode,
        COMPOSED_OPERATIONS,
    );

/// Whether a validated RGB PutPixel prefix can use Thumbnail's sparse 2x2 path.
pub(crate) fn rgb_putpixel_fusion_supported(
    image: &DynamicImage,
    writes: &[PipelineOp],
    thumbnail: &PipelineOp,
    context: &ExecutionContext,
) -> bool {
    if context.backend() != Backend::Cpu
        || !matches!(image, DynamicImage::ImageRgb8(_))
    {
        return false;
    }
    if writes.is_empty()
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
    let PipelineOp::Thumbnail {
        w,
        h,
        filter,
        reducing_gap,
    } = thumbnail else {
        return false;
    };
    if *reducing_gap != Some(2.0) {
        return false;
    }
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

/// One admitted RGB PutPixel/Thumbnail fusion with a prepared Resize strategy.
pub(crate) struct PreparedRgbPutPixelThumbnail<'a> {
    image: &'a DynamicImage,
    writes: &'a [PipelineOp],
    thumbnail: &'a PipelineOp,
    context: ExecutionContext,
    resize_selection: StrategySelection,
}

impl PreparedRgbPutPixelThumbnail<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.resize_selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        super::rgb8::ops::execute_putpixel_thumbnail(
            self.image,
            self.writes,
            self.thumbnail,
            &self.context,
            self.resize_selection,
        )
    }
}

/// Select the supported fusion and its exact child Resize strategy.
pub(crate) fn prepare_rgb_putpixel_thumbnail<'a>(
    image: &'a DynamicImage,
    writes: &'a [PipelineOp],
    thumbnail: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<Option<PreparedRgbPutPixelThumbnail<'a>>, PilError> {
    if !rgb_putpixel_fusion_supported(image, writes, thumbnail, context) {
        return Ok(None);
    }
    let inputs = OperationInputs {
        image,
        palette: crate::compute::registry::support::PaletteInput::default(),
    };
    let prepared = prepare_pipeline(inputs, thumbnail, context)?;
    let boxed_resize_selection = prepared
        .composed_selections()
        .iter()
        .find(|selection| {
            selection.implementation.operation == OperationId::ResizeBoxed
                && selection.implementation.mode == Mode::ImageRgb8
        })
        .copied()
        .ok_or_else(|| {
            PilError::InternalError(
                "RGB Thumbnail fusion has no prepared boxed Resize strategy".into(),
            )
        })?;
    let PipelineOp::Thumbnail {
        w,
        h,
        filter,
        reducing_gap,
    } = thumbnail else {
        return Err(PilError::InternalError(
            "prepared RGB Thumbnail changed after admission".into(),
        ));
    };
    if *reducing_gap != Some(2.0) {
        return Err(PilError::InternalError(
            "prepared RGB Thumbnail changed after admission".into(),
        ));
    }
    let reduced_facts = reduced_resize_facts(inputs, (2, 2))?.with_resize(
        (*w).max(1).min(inputs.image.width()),
        (*h).max(1).min(inputs.image.height()),
        *filter,
        false,
    );
    let unboxed_resize_selection = select_child_mode(
        &crate::compute::pool_cpu::ops::resize::ops::SUPPORT,
        Mode::ImageRgb8,
        reduced_facts,
        context,
    )?;
    let resize_selection = if unboxed_resize_selection.implementation.architecture
        == Architecture::Default
    {
        boxed_resize_selection
    } else {
        unboxed_resize_selection
    };
    Ok(Some(PreparedRgbPutPixelThumbnail {
        image,
        writes,
        thumbnail,
        context: *context,
        resize_selection,
    }))
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
    use_unboxed_resize: bool,
    stream_i32: bool,
    context: ExecutionContext,
    selection: StrategySelection,
    reduce_plan: Option<crate::compute::pool_cpu::ops::reduce::ops::PreparedReduce<'a>>,
    resize_selection: Option<StrategySelection>,
    composed_selections: Vec<StrategySelection>,
}

struct ThumbnailChildren<'a> {
    reduce_plan: Option<crate::compute::pool_cpu::ops::reduce::ops::PreparedReduce<'a>>,
    resize_selection: Option<StrategySelection>,
    selections: Vec<StrategySelection>,
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

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        &self.composed_selections
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
    reducing_gap: Option<f64>,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match SUPPORT
        .for_mode(mode)
        .assess(
            context,
            inputs
                .facts()
                .with_thumbnail(width, height, filter, reducing_gap),
        )?
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
    let PipelineOp::Thumbnail {
        w,
        h,
        filter,
        reducing_gap,
    } = op else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    let mode = inputs.mode();
    let mode_strategy = ThumbnailModeStrategy::for_mode(mode);
    let effective_filter = mode_strategy.effective_filter(*filter);
    let selection = select_mode(inputs, mode, *w, *h, *filter, *reducing_gap, context)?;
    let reduce = mode_strategy.should_reduce(effective_filter, *reducing_gap);
    let source = (inputs.image.width(), inputs.image.height());
    let output = ((*w).max(1).min(source.0), (*h).max(1).min(source.1));
    let factors = crate::compute::helpers::resampling::thumbnail::reduce_factors_with_gap(
        source,
        output,
        if reduce { *reducing_gap } else { None },
    )?;
    let stream_i32 = *w > 0
        && *h > 0
        && mode_strategy.streams_i32_thumbnail(source, output, effective_filter, factors);
    let use_unboxed_resize = factors == (1, 1)
        || mode_strategy.uses_unboxed_resize(source, output, factors);
    let resize_request = ThumbnailResizeRequest {
        source,
        output,
        factors,
        filter: effective_filter,
        use_unboxed_resize,
    };
    let children = prepare_composed_strategies(
        inputs,
        mode,
        resize_request,
        stream_i32,
        context,
    )?;
    Ok(crate::compute::registry::PreparedOperation::from_thumbnail(
        PreparedThumbnail {
            inputs,
            width: *w,
            height: *h,
            source,
            output,
            factors,
            effective_filter,
            use_unboxed_resize,
            stream_i32,
            context: *context,
            selection,
            reduce_plan: children.reduce_plan,
            resize_selection: children.resize_selection,
            composed_selections: children.selections,
        },
    ))
}

fn prepare_composed_strategies<'a>(
    inputs: OperationInputs<'a>,
    mode: Mode,
    request: ThumbnailResizeRequest,
    stream_i32: bool,
    context: &ExecutionContext,
) -> Result<ThumbnailChildren<'a>, PilError> {
    if stream_i32 {
        return Ok(ThumbnailChildren {
            reduce_plan: None,
            resize_selection: None,
            selections: Vec::new(),
        });
    }

    let reduce_plan = if request.factors != (1, 1) {
        Some(crate::compute::pool_cpu::ops::reduce::ops::prepare(
            inputs,
            request.factors,
            context,
        )?)
    } else {
        None
    };
    let mut selections = Vec::with_capacity(2);
    if let Some(plan) = &reduce_plan {
        selections.push(*plan.selection());
    }

    let mut resize_facts = reduced_resize_facts(inputs, request.factors)?;
    resize_facts = match request.use_unboxed_resize {
        true => resize_facts.with_resize(
            request.output.0,
            request.output.1,
            request.filter,
            false,
        ),
        false => resize_facts.with_boxed_resize(
            request.output.0,
            request.output.1,
            request.filter,
            (
                0.0,
                0.0,
                f64::from(request.source.0) / f64::from(request.factors.0),
                f64::from(request.source.1) / f64::from(request.factors.1),
            ),
        ),
    };
    let support = match request.use_unboxed_resize {
        true => &crate::compute::pool_cpu::ops::resize::ops::SUPPORT,
        false => &crate::compute::pool_cpu::ops::resize::ops::BOXED_SUPPORT,
    };
    let resize_selection = select_child_mode(support, mode, resize_facts, context)?;
    selections.push(resize_selection);
    Ok(ThumbnailChildren {
        reduce_plan,
        resize_selection: Some(resize_selection),
        selections,
    })
}

fn reduced_resize_facts(
    inputs: OperationInputs<'_>,
    factors: (u32, u32),
) -> Result<crate::compute::registry::support::StrategyInputs, PilError> {
    let mut facts = inputs.facts();
    if factors == (1, 1) {
        return Ok(facts);
    }

    let width = facts.width.div_ceil(factors.0);
    let height = facts.height.div_ceil(factors.1);
    let source_pixels = (facts.width as usize).checked_mul(facts.height as usize);
    let output_pixels = (width as usize).checked_mul(height as usize);
    facts.byte_len = match (source_pixels, output_pixels, facts.byte_len) {
        (Some(0), _, Some(_)) => Some(0),
        (Some(source_pixels), Some(output_pixels), Some(source_bytes)) => {
            let bytes_per_pixel = source_bytes
                .checked_div(source_pixels)
                .filter(|_| source_bytes.is_multiple_of(source_pixels))
                .ok_or_else(|| {
                    PilError::InternalError("Thumbnail source layout has no pixel size".into())
                })?;
            Some(output_pixels.checked_mul(bytes_per_pixel).ok_or_else(|| {
                PilError::InternalError("Thumbnail reduced byte length overflowed".into())
            })?)
        }
        _ => {
            return Err(PilError::InternalError(
                "Thumbnail reduced layout facts are unresolved".into(),
            ));
        }
    };
    facts.width = width;
    facts.height = height;
    Ok(facts)
}

fn select_child_mode(
    support: &'static OperationSupport,
    mode: Mode,
    facts: crate::compute::registry::support::StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Thumbnail child strategy remained unresolved after materialization".into(),
        )),
    }
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

    let factors = if plan.factors == (1, 1) {
        (1, 1)
    } else {
        plan.factors
    };
    let work_image = match plan.reduce_plan {
        Some(reduce_plan) => reduce_plan.execute()?,
        None => image.clone(),
    };
    let resize_selection = plan.resize_selection.ok_or_else(|| {
        PilError::InternalError("Thumbnail is missing its prepared Resize strategy".into())
    })?;
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
        resize_selection,
        &plan.context,
    )
}

fn resize_thumbnail_work_image(
    image: &DynamicImage,
    palette: crate::compute::registry::support::PaletteInput<'_>,
    request: ThumbnailResizeRequest,
    selection: StrategySelection,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    let inputs = OperationInputs { image, palette };
    let bounds = (!request.use_unboxed_resize).then_some((
        0.0,
        0.0,
        f64::from(request.source.0) / f64::from(request.factors.0),
        f64::from(request.source.1) / f64::from(request.factors.1),
    ));
    crate::compute::pool_cpu::ops::resize::ops::prepare_direct_with_selection(
        inputs,
        request.output.0,
        request.output.1,
        request.filter,
        bounds,
        context,
        selection,
    )?
    .execute()
}

