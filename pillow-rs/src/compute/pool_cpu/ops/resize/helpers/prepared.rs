//! Prepared Resize mode and architecture execution plans.

use super::{BOXED_SUPPORT, SUPPORT};
use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        ModeSupport, OperationId, OperationInputs, OperationSupport, StrategyInputs,
        StrategySelection, SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

pub(crate) const fn effective_filter(mode: Mode, requested: ResampleFilter) -> ResampleFilter {
    match mode {
        Mode::One => super::super::one::ops::effective_filter(requested),
        Mode::ImagePalette8 => super::super::palette8::ops::effective_filter(requested),
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
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
        | Mode::ImageRgba32F => requested,
    }
}

#[derive(Clone, Copy)]
enum ResizeRequest {
    Standard {
        filter: ResampleFilter,
    },
    Boxed {
        filter: ResampleFilter,
        bounds: (f64, f64, f64, f64),
        reducing_gap: Option<f64>,
    },
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_alpha_reducing_gap(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLumaA8 | Mode::ImageRgba8 => ModeSupport::serial(
            OperationId::ResizeBoxed,
            mode,
            super::super::helpers::support::INPUTS,
        ),
        _ => super::classify_boxed_resize(mode),
    }
}

static ALPHA_REDUCING_GAP_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ResizeBoxed,
    super::super::helpers::support::INPUTS,
    classify_alpha_reducing_gap,
);

pub(crate) const fn alpha_reducing_gap_support() -> &'static OperationSupport {
    &ALPHA_REDUCING_GAP_SUPPORT
}

/// Exact mode and architecture choice retained through execution.
pub(crate) struct PreparedResize<'a> {
    image: &'a DynamicImage,
    context: ExecutionContext,
    width: u32,
    height: u32,
    request: ResizeRequest,
    selection: StrategySelection,
}

impl PreparedResize<'_> {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn supports_pad_window(&self) -> bool {
        let filtered = matches!(
            self.request,
            ResizeRequest::Standard { filter } if !matches!(filter, ResampleFilter::Nearest)
        );
        super::super::helpers::pad_window::supports(self.image, &self.selection, filtered)
    }

    /// Executes the retained mode and architecture strategy into a Pad window.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute_into_pad_window(&self, output: &mut [u8]) -> Result<bool, PilError> {
        if !self.supports_pad_window() {
            return Ok(false);
        }
        let ResizeRequest::Standard { filter } = self.request else {
            return Ok(false);
        };
        super::super::helpers::pad_window::execute(
            self.image,
            self.width,
            self.height,
            filter,
            &self.selection,
            output,
        )
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        if let ResizeRequest::Boxed {
            filter,
            bounds,
            reducing_gap: Some(gap),
        } = self.request
        {
            return execute_alpha_reducing_gap(
                self.image,
                (self.width, self.height),
                filter,
                bounds,
                gap,
                &self.selection,
            );
        }
        if let ResizeRequest::Boxed { filter, bounds, .. } = self.request
            && let Some(mut output) = super::super::helpers::tall_boxed::execute(
                self.image,
                &self.context,
                (self.width, self.height),
                filter,
                bounds,
            )?
        {
            super::super::helpers::hidden_lanes::finish(
                self.image,
                &mut output,
                filter,
                Some(bounds),
            )?;
            return Ok(output);
        }
        if self.selection.implementation.architecture == Architecture::Default
            && let ResizeRequest::Boxed { filter, bounds, .. } = self.request
            && let Some(cropped) = super::super::helpers::boxed_crop::execute_default(
                self.image,
                (self.width, self.height),
                filter,
                bounds,
            )
        {
            return Ok(cropped);
        }
        let (filter, bounds) = match self.request {
            ResizeRequest::Standard { filter } => (filter, None),
            ResizeRequest::Boxed { filter, bounds, .. } => (filter, Some(bounds)),
        };
        let mut output = execute_mode(
            self.image,
            self.context,
            self.width,
            self.height,
            filter,
            bounds,
            self.selection,
        )?;
        super::super::helpers::hidden_lanes::finish(self.image, &mut output, filter, bounds)?;
        Ok(output)
    }

    /// Executes the prepared RGB Resize strategy with exact active source-row
    /// facts supplied by Thumbnail's sparse reducing-gap fusion.
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute_rgb8_with_sparse_rows(
        self,
        sparse_source_rows: Option<&[bool]>,
    ) -> Result<DynamicImage, PilError> {
        if let ResizeRequest::Boxed { bounds, .. } = self.request
            && sparse_source_rows.is_some()
            && super::super::helpers::tall_boxed::required(
                self.image,
                (self.width, self.height),
                bounds,
            )
        {
            return self.execute();
        }
        let Some(sparse_source_rows) = sparse_source_rows else {
            return self.execute();
        };
        let (filter, bounds) = match self.request {
            ResizeRequest::Standard { filter } => (filter, None),
            ResizeRequest::Boxed { filter, bounds, .. } => (filter, Some(bounds)),
        };
        let expected_operation = if bounds.is_some() {
            OperationId::ResizeBoxed
        } else {
            OperationId::Resize
        };
        if self.selection.implementation.operation != expected_operation
            || self.selection.implementation.mode != Mode::ImageRgb8
            || self.selection.requested_backend != self.context.backend()
            || self.selection.requested_architecture != self.context.architecture()
        {
            return Err(PilError::InternalError(
                "sparse RGB Thumbnail resize received an incompatible selection".into(),
            ));
        }
        let DynamicImage::ImageRgb8(image) = self.image else {
            return Err(PilError::InternalError(
                "sparse RGB Thumbnail resize received a different mode".into(),
            ));
        };
        let output = super::super::rgb8::ops::resize_with_sparse_rows(
            &self.context,
            &self.selection,
            image,
            self.width,
            self.height,
            bounds,
            filter,
            sparse_source_rows,
        )?;
        let mut output = DynamicImage::ImageRgb8(output);
        super::super::helpers::hidden_lanes::finish(self.image, &mut output, filter, bounds)?;
        Ok(output)
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn execute_alpha_reducing_gap(
    image: &DynamicImage,
    destination: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    gap: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "straight-alpha reducing-gap Resize requires its Default strategy".into(),
        ));
    }
    match image {
        DynamicImage::ImageLumaA8(input) => {
            let bytes = super::super::helpers::alpha_samples::resize_with_reducing_gap::<2>(
                input.as_raw(),
                input.dimensions(),
                destination,
                filter,
                bounds,
                gap,
            )?;
            crate::raster::GrayAlphaImage::from_raw(destination.0, destination.1, bytes)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| {
                    PilError::ValueError("resize_la: failed to create output buffer".into())
                })
        }
        DynamicImage::ImageRgba8(input) => {
            let bytes = super::super::helpers::alpha_samples::resize_with_reducing_gap::<4>(
                input.as_raw(),
                input.dimensions(),
                destination,
                filter,
                bounds,
                gap,
            )?;
            crate::raster::RgbaImage::from_raw(destination.0, destination.1, bytes)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| {
                    PilError::ValueError("resize_rgba: failed to create output buffer".into())
                })
        }
        _ => Err(PilError::InternalError(
            "straight-alpha reducing-gap Resize received a different raster mode".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn execute_mode(
    image: &DynamicImage,
    context: ExecutionContext,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match image.mode() {
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
        | Mode::ImageHsv8 => super::super::helpers::mode_dispatch::execute_byte_mode(
            image, &context, &selection, width, height, filter, bounds,
        ),
        Mode::ImageI32 => super::super::i32::ops::resize(
            &context, image, width, height, filter, bounds, &selection,
        ),
        Mode::ImageF32 => super::super::f32::ops::resize(
            &context, image, width, height, filter, bounds, &selection,
        ),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::helpers::mode_dispatch::execute_luma16_mode(
                image, &context, &selection, width, height, filter, bounds,
            )
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => super::super::helpers::mode_dispatch::execute_typed_mode(
            image, &context, &selection, width, height, filter, bounds,
        ),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn selection(
    support: &'static OperationSupport,
    mode: Mode,
    facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.for_mode(mode).assess(ctx, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Resize support remained unresolved after materialization".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn resize_request_facts(
    facts: StrategyInputs,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> StrategyInputs {
    match bounds {
        Some(bounds) => facts.with_boxed_resize(width, height, filter, bounds),
        None => facts.with_resize(width, height, filter, false),
    }
}

/// Prepares a typed Resize dependency for another CPU operation.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_direct<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    ctx: &ExecutionContext,
) -> Result<PreparedResize<'a>, PilError> {
    let filter = effective_filter(inputs.mode(), filter);
    let bounds = match bounds {
        Some(bounds) => Some(super::super::helpers::boxed_crop::validate_bounds(
            bounds,
            (inputs.image.width(), inputs.image.height()),
        )?),
        None => None,
    };
    let boxed = bounds.is_some();
    let support = if boxed { &BOXED_SUPPORT } else { &SUPPORT };
    let facts = resize_request_facts(inputs.facts(), width, height, filter, bounds);
    let selected = selection(support, inputs.mode(), facts, ctx)?;
    prepare_direct_with_selection(inputs, width, height, filter, bounds, ctx, selected)
}

/// Binds a previously assessed exact-mode Resize selection to its image input.
///
/// Composite operations use this after materializing an intermediate raster;
/// the stored support decision is retained rather than selected a second time.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_direct_with_selection<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    ctx: &ExecutionContext,
    selected: StrategySelection,
) -> Result<PreparedResize<'a>, PilError> {
    prepare_direct_with_request(inputs, width, height, filter, bounds, None, ctx, selected)
}

/// Binds a prepared straight-alpha reducing-gap strategy to its image input.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_direct_with_reducing_gap_selection<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    reducing_gap: f64,
    ctx: &ExecutionContext,
    selected: StrategySelection,
) -> Result<PreparedResize<'a>, PilError> {
    prepare_direct_with_request(
        inputs,
        width,
        height,
        filter,
        Some(bounds),
        Some(reducing_gap),
        ctx,
        selected,
    )
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn prepare_direct_with_request<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    reducing_gap: Option<f64>,
    ctx: &ExecutionContext,
    selected: StrategySelection,
) -> Result<PreparedResize<'a>, PilError> {
    if effective_filter(inputs.mode(), filter) != filter {
        return Err(PilError::InternalError(
            "prepared Resize selection uses a non-effective mode filter".into(),
        ));
    }
    if reducing_gap.is_some() && !matches!(inputs.mode(), Mode::ImageLumaA8 | Mode::ImageRgba8) {
        return Err(PilError::InternalError(
            "prepared reducing-gap Resize requires a straight-alpha mode".into(),
        ));
    }
    let bounds = match bounds {
        Some(bounds) => Some(super::super::helpers::boxed_crop::validate_bounds(
            bounds,
            (inputs.image.width(), inputs.image.height()),
        )?),
        None => None,
    };
    let expected_operation = if bounds.is_some() || reducing_gap.is_some() {
        OperationId::ResizeBoxed
    } else {
        OperationId::Resize
    };
    if selected.implementation.operation != expected_operation
        || selected.implementation.mode != inputs.mode()
        || selected.requested_backend != ctx.backend()
        || selected.requested_architecture != ctx.architecture()
    {
        return Err(PilError::InternalError(
            "prepared Resize selection does not match its bound request".into(),
        ));
    }
    let request = match bounds {
        Some(bounds) => ResizeRequest::Boxed {
            filter,
            bounds,
            reducing_gap,
        },
        None => ResizeRequest::Standard { filter },
    };
    Ok(PreparedResize {
        image: inputs.image,
        context: *ctx,
        width,
        height,
        request,
        selection: selected,
    })
}

/// Prepares ordinary or boxed Resize using the operation's declared support.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let (width, height, filter, request, support) = match op {
        PipelineOp::Resize { w, h, filter } => (
            *w,
            *h,
            *filter,
            ResizeRequest::Standard { filter: *filter },
            &SUPPORT,
        ),
        PipelineOp::ResizeBoxed {
            w,
            h,
            filter,
            box_coords,
            reducing_gap,
        } => (
            *w,
            *h,
            *filter,
            ResizeRequest::Boxed {
                filter: *filter,
                bounds: *box_coords,
                reducing_gap: *reducing_gap,
            },
            if reducing_gap.is_some() {
                if !matches!(inputs.mode(), Mode::ImageLumaA8 | Mode::ImageRgba8) {
                    return Err(PilError::InternalError(
                        "reducing-gap ResizeBoxed is limited to straight-alpha modes".into(),
                    ));
                }
                &ALPHA_REDUCING_GAP_SUPPORT
            } else {
                &BOXED_SUPPORT
            },
        ),
        _ => return Err(PilError::ValueError("expected Resize op".into())),
    };
    let filter = effective_filter(inputs.mode(), filter);
    let request = match request {
        ResizeRequest::Standard { .. } => ResizeRequest::Standard { filter },
        ResizeRequest::Boxed {
            bounds,
            reducing_gap,
            ..
        } => ResizeRequest::Boxed {
            filter,
            bounds: super::super::helpers::boxed_crop::validate_bounds(
                bounds,
                (inputs.image.width(), inputs.image.height()),
            )?,
            reducing_gap,
        },
    };
    let bounds = match request {
        ResizeRequest::Standard { .. } => None,
        ResizeRequest::Boxed { bounds, .. } => Some(bounds),
    };
    let facts = resize_request_facts(inputs.facts(), width, height, filter, bounds);
    let selected = selection(support, inputs.mode(), facts, ctx)?;
    let plan = PreparedResize {
        image: inputs.image,
        context: *ctx,
        width,
        height,
        request,
        selection: selected,
    };
    Ok(crate::compute::registry::PreparedOperation::from_resize(
        plan,
    ))
}
