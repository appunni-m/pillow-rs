//! Prepared Resize mode and architecture execution plans.

use super::{BOXED_SUPPORT, SUPPORT};
use crate::checked_dims::CheckedDims;
use crate::compute::{
    Architecture, ExecutionContext,
    registry::support::{
        OperationId, OperationInputs, OperationSupport, StrategyInputs, StrategySelection,
        SupportAssessment,
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
    Standard { filter: ResampleFilter },
    Boxed { filter: ResampleFilter, bounds: (f64, f64, f64, f64) },
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
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn supports_pad_window(&self) -> bool {
        let filtered = matches!(
            self.request,
            ResizeRequest::Standard { filter } if !matches!(filter, ResampleFilter::Nearest)
        );
        super::super::helpers::pad_window::supports(
            self.image,
            self.selection.implementation.architecture,
            filtered,
        )
    }

    /// Executes the retained mode and architecture strategy into a Pad window.
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
            self.selection.implementation.architecture,
            output,
        )
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        if let ResizeRequest::Boxed { filter, bounds } = self.request
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
            && let ResizeRequest::Boxed { filter, bounds } = self.request
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
            ResizeRequest::Boxed { filter, bounds } => (filter, Some(bounds)),
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
            ResizeRequest::Boxed { filter, bounds } => (filter, Some(bounds)),
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
            self.selection.implementation.architecture,
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
            image,
            &context,
            selection.implementation.architecture,
            width,
            height,
            filter,
            bounds,
        ),
        Mode::ImageI32 => super::super::i32::ops::resize(
            &context,
            image,
            width,
            height,
            filter,
            bounds,
            selection.implementation.architecture,
        ),
        Mode::ImageF32 => super::super::f32::ops::resize(
            &context,
            image,
            width,
            height,
            filter,
            bounds,
            selection.implementation.architecture,
        ),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::helpers::mode_dispatch::execute_luma16_mode(
                image,
                &context,
                selection.implementation.architecture,
                width,
                height,
                filter,
                bounds,
            )
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            super::super::helpers::mode_dispatch::execute_typed_mode(
                image,
                &context,
                selection.implementation.architecture,
                width,
                height,
                filter,
                bounds,
            )
        }
    }
}

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
pub(crate) fn prepare_direct_with_selection<'a>(
    inputs: OperationInputs<'a>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
    ctx: &ExecutionContext,
    selected: StrategySelection,
) -> Result<PreparedResize<'a>, PilError> {
    if effective_filter(inputs.mode(), filter) != filter {
        return Err(PilError::InternalError(
            "prepared Resize selection uses a non-effective mode filter".into(),
        ));
    }
    let bounds = match bounds {
        Some(bounds) => Some(super::super::helpers::boxed_crop::validate_bounds(
            bounds,
            (inputs.image.width(), inputs.image.height()),
        )?),
        None => None,
    };
    let expected_operation = if bounds.is_some() {
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
        Some(bounds) => ResizeRequest::Boxed { filter, bounds },
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
        PipelineOp::ResizeBoxed { w, h, filter, box_coords } => (
            *w,
            *h,
            *filter,
            ResizeRequest::Boxed { filter: *filter, bounds: *box_coords },
            &BOXED_SUPPORT,
        ),
        _ => return Err(PilError::ValueError("expected Resize op".into())),
    };
    let filter = effective_filter(inputs.mode(), filter);
    let request = match request {
        ResizeRequest::Standard { .. } => ResizeRequest::Standard { filter },
        ResizeRequest::Boxed { bounds, .. } => ResizeRequest::Boxed {
            filter,
            bounds: super::super::helpers::boxed_crop::validate_bounds(
                bounds,
                (inputs.image.width(), inputs.image.height()),
            )?,
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
    Ok(crate::compute::registry::PreparedOperation::from_resize(plan))
}

pub(crate) fn native_luma16_resize(
    image: &DynamicImage,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Option<DynamicImage> {
    match image {
        DynamicImage::ImageI16(input) => Some(super::super::i16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16L(input) => Some(super::super::i16l::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16B(input) => Some(super::super::i16b::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageI16N(input) => Some(super::super::i16n::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        _ => None,
    }
}

pub(crate) fn native_typed_resize(
    image: &DynamicImage,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Option<DynamicImage> {
    match image {
        DynamicImage::ImageLumaA16(input) => Some(super::super::luma_a16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgb16(input) => Some(super::super::rgb16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgba16(input) => Some(super::super::rgba16::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgb32F(input) => Some(super::super::rgb32f::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        DynamicImage::ImageRgba32F(input) => Some(super::super::rgba32f::ops::resize(
            input,
            destination_width,
            destination_height,
            filter,
            bounds,
        )),
        _ => None,
    }
}
