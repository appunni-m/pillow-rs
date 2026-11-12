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

use super::helpers::pad_dimensions::PadFrame;
use super::helpers::{contain_dimensions, cover_dimensions, fit_box, pad_dimensions as dimensions};
use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, OutputModeRule,
        PadFillRule, StrategyInputs, StrategySelection, SupportAssessment,
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_contain(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Contain, CONTAIN_INPUTS, mode)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_cover(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Cover, COVER_INPUTS, mode)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_fit(mode: Mode) -> ModeSupport {
    classify_mode(OperationId::Fit, FIT_INPUTS, mode)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        match &self.work {
            GeometryWork::Copy(_) => &[],
            GeometryWork::Resize(resize) => std::slice::from_ref(resize.selection()),
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
            prepare_pad_pipeline(inputs, op, context)
        }
        PipelineOp::Scale { factor, filter } => prepare_scale(inputs, *factor, *filter, context)
            .map(crate::compute::registry::PreparedOperation::from_scale),
        _ => Err(PilError::ValueError("expected a geometry op".into())),
    }
}

#[derive(PartialEq, Eq)]
pub(crate) enum PadContentSource {
    Input,
    Resized,
}

const PAD_PARAMETERS_INPUTS: &[InputContract] = &[InputContract::PadParameters];
pub(super) const MODE_ENCODED_FILL_INPUTS: &[InputContract] = &[
    InputContract::PadParameters,
    InputContract::PadFill(PadFillRule::ModeEncodedBytes),
];
pub(super) const PALETTE_FILL_INPUTS: &[InputContract] = &[
    InputContract::PadParameters,
    InputContract::PadFill(PadFillRule::PaletteIndexOrColor),
];
pub(super) const NATIVE_U16_FILL_INPUTS: &[InputContract] = &[
    InputContract::PadParameters,
    InputContract::PadFill(PadFillRule::NativeU16),
];
pub(super) const NATIVE_F32_FILL_INPUTS: &[InputContract] = &[
    InputContract::PadParameters,
    InputContract::PadFill(PadFillRule::NativeF32),
];
const PAD_RESIZE_CHILD: &[OperationId] = &[OperationId::Resize];

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_pad_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SUPPORT,
        Mode::ImageI32 => super::i32::ops::SUPPORT,
        Mode::ImageF32 => super::f32::ops::SUPPORT,
        Mode::ImageI16 => super::i16::ops::SUPPORT,
        Mode::ImageI16L => super::i16l::ops::SUPPORT,
        Mode::ImageI16B => super::i16b::ops::SUPPORT,
        Mode::ImageI16N => super::i16n::ops::SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
    }
}

/// Exhaustive Pad support for all exact raster identities.
pub(crate) static PAD_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Pad,
    PAD_PARAMETERS_INPUTS,
    classify_pad_mode,
    PAD_RESIZE_CHILD,
);

/// Prepared Pad retaining its exact mode and selected Resize child.
pub(crate) struct PreparedPad<'a> {
    inputs: OperationInputs<'a>,
    frame: PadFrame,
    resize: Option<crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'a>>,
    selection: StrategySelection,
}

impl PreparedPad<'_> {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        self.resize
            .as_ref()
            .map_or(&[], |resize| std::slice::from_ref(resize.selection()))
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let source = self.inputs.image;
        let frame = self.frame;
        let selection = &self.selection;
        let architecture = selection.implementation.architecture;
        if let Some(padded) =
            try_direct_resize_into_canvas(source, frame, self.resize.as_ref(), selection)?
        {
            if let Some(resize) = self.resize.as_ref() {
                crate::compute::record_pipeline_operation_selection(resize.selection());
            }
            return finish_pad(source, padded, frame);
        }
        let resized = match self.resize {
            Some(resize) => {
                crate::compute::record_pipeline_operation_selection(resize.selection());
                Some(resize.execute()?)
            }
            None => None,
        };
        let content_source = if resized.is_some() {
            PadContentSource::Resized
        } else {
            PadContentSource::Input
        };
        let content = resized.as_ref().unwrap_or(source);
        if let Some((image, vector_blocks, scalar_tail)) =
            try_vertical_append(content, frame, selection, content_source)?
        {
            record_vector_work(vector_blocks, scalar_tail);
            return finish_pad(content, image, frame);
        }
        if architecture == Architecture::Default
            && frame.content_width == frame.width
            && frame.content_height == frame.height
            && frame.offset_x == 0
            && frame.offset_y == 0
        {
            return finish_pad(content, content.clone(), frame);
        }
        let (image, vector_blocks, scalar_tail) = pad_mode(content, frame, selection)?;
        record_vector_work(vector_blocks, scalar_tail);
        finish_pad(content, image, frame)
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn finish_pad(
    source: &DynamicImage,
    mut output: DynamicImage,
    frame: PadFrame,
) -> Result<DynamicImage, PilError> {
    super::helpers::pad_hidden_lanes::preserve_same_size_content(source, &mut output, frame)?;
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn record_vector_work(vector_blocks: u64, scalar_tail: u64) {
    if vector_blocks != 0 || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn try_vertical_append(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
    content_source: PadContentSource,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    match image.mode() {
        Mode::ImageLuma8 => {
            super::luma8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        Mode::ImageRgb8 => {
            super::rgb8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        Mode::ImageRgba8 => {
            super::rgba8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        Mode::ImageCmyk8 => {
            super::cmyk8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        Mode::ImageHsv8 => {
            super::hsv8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        Mode::ImageRgbx8 => {
            super::rgbx8::ops::try_vertical_append(image, frame, selection, content_source)
        }
        mode => {
            super::helpers::pad_strategies::selected_architecture(selection, mode)?;
            Ok(None)
        }
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn try_direct_resize_into_canvas(
    image: &DynamicImage,
    frame: PadFrame,
    resize: Option<&crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'_>>,
    selection: &StrategySelection,
) -> Result<Option<DynamicImage>, PilError> {
    if resize.is_none() {
        return Ok(None);
    }
    match image.mode() {
        Mode::ImageLuma8 => {
            super::luma8::ops::try_direct_resize_into_canvas(image, frame, resize, selection)
        }
        Mode::ImageRgbx8 => {
            super::rgbx8::ops::try_direct_resize_into_canvas(image, frame, resize, selection)
        }
        mode => {
            super::helpers::pad_strategies::selected_architecture(selection, mode)?;
            Ok(None)
        }
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn select_pad(
    mode: Mode,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match PAD_SUPPORT.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Pad preparation has unresolved input facts".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn pad_mode(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<(DynamicImage, u64, u64), PilError> {
    match image.mode() {
        Mode::One => super::one::ops::pad(image, frame, selection),
        Mode::ImageLuma8 => super::luma8::ops::pad(image, frame, selection),
        Mode::ImageLumaA8 => super::luma_a8::ops::pad(image, frame, selection),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImagePalette8 => super::palette8::ops::pad(image, frame, selection),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::pad(image, frame, selection),
        Mode::ImageRgb8 => super::rgb8::ops::pad(image, frame, selection),
        Mode::ImageRgba8 => super::rgba8::ops::pad(image, frame, selection),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::pad(image, frame, selection),
        Mode::ImageCmyk8 => super::cmyk8::ops::pad(image, frame, selection),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::pad(image, frame, selection),
        Mode::ImageLab8 => {
            super::lab8::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageHsv8 => super::hsv8::ops::pad(image, frame, selection),
        Mode::ImageI32 => super::i32::ops::pad(image, frame, selection),
        Mode::ImageF32 => super::f32::ops::pad(image, frame, selection),
        Mode::ImageI16 => super::i16::ops::pad(image, frame, selection).map(|image| (image, 0, 0)),
        Mode::ImageI16L => {
            super::i16l::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageI16B => {
            super::i16b::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageI16N => {
            super::i16n::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageLumaA16 => {
            super::luma_a16::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageRgb16 => {
            super::rgb16::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageRgba16 => {
            super::rgba16::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageRgb32F => {
            super::rgb32f::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
        Mode::ImageRgba32F => {
            super::rgba32f::ops::pad(image, frame, selection).map(|image| (image, 0, 0))
        }
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_pad_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let (w, h, filter, color, native_color, centering, position) = match op {
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        } => (w, h, filter, color, None, *centering, None),
        PipelineOp::PadTyped {
            w,
            h,
            filter,
            color,
            input,
            centering,
        } => (w, h, filter, color, Some(*input), *centering, None),
        PipelineOp::PadPositioned {
            w,
            h,
            filter,
            color,
            native_color,
            position,
        } => (
            w,
            h,
            filter,
            color,
            *native_color,
            (0.5, 0.5),
            Some(*position),
        ),
        _ => return Err(PilError::ValueError("expected Pad op".into())),
    };
    let mode = inputs.mode();
    let facts = inputs.facts().with_pad(*w, *h, *filter, *color, centering);
    // Pillow ImageOps.pad computes its contain geometry before it reaches the
    // resize/fill strategy. Keep direct prepared calls on that error order.
    let mut frame = dimensions::build(
        (inputs.image.width(), inputs.image.height()),
        (*w, *h),
        centering,
        *color,
    )?;
    let selection = select_pad(mode, facts, context)?;
    if let Some((x, y)) = position {
        frame.offset_x = i64::from(x);
        frame.offset_y = i64::from(y);
    }
    frame.native_color = native_color;
    let native_identity = matches!(
        mode,
        Mode::ImageLuma8
            | Mode::ImageLumaA8
            | Mode::ImageRgb8
            | Mode::ImageHsv8
            | Mode::ImageRgba8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
    ) && (inputs.image.width(), inputs.image.height())
        == (frame.content_width, frame.content_height);
    let resize = if frame.content_width == 0 || frame.content_height == 0 || native_identity {
        None
    } else {
        Some(crate::compute::pool_cpu::ops::resize::ops::prepare_direct(
            inputs,
            frame.content_width,
            frame.content_height,
            *filter,
            None,
            context,
        )?)
    };
    Ok(crate::compute::registry::PreparedOperation::from_pad(
        PreparedPad {
            inputs,
            frame,
            resize,
            selection,
        },
    ))
}
