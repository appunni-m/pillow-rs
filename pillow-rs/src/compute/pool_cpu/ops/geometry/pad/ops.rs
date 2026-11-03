//! Pad support declarations and prepared exact-mode Resize composition.
// TODO(pillow-rs#3ptb): verify public Pad results and errors for custom,
// nonnumeric, and wide tuple values; verify P palette output, native fills,
// and composed Resize in CPU parity.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, PadFillRule,
    OutputModeRule, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

use super::helpers::dimensions::{self, PadFrame};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PadContentSource {
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
const RESIZE_CHILD: &[OperationId] = &[OperationId::Resize];

fn classify_mode(mode: Mode) -> ModeSupport {
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
pub(crate) static SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::Pad,
    PAD_PARAMETERS_INPUTS,
    classify_mode,
    RESIZE_CHILD,
);

/// Prepared Pad retaining its exact mode and selected Resize child.
pub(crate) struct PreparedPad<'a> {
    inputs: OperationInputs<'a>,
    frame: PadFrame,
    resize: Option<crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'a>>,
    selection: StrategySelection,
}

impl PreparedPad<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn composed_selections(&self) -> &[StrategySelection] {
        self.resize
            .as_ref()
            .map_or(&[], |resize| std::slice::from_ref(resize.selection()))
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let source = self.inputs.image;
        let frame = self.frame;
        let architecture = self.selection.implementation.architecture;
        if let Some(padded) = try_direct_resize_into_canvas(
            source,
            frame,
            self.resize.as_ref(),
            architecture,
        )? {
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
        if let Some((image, vector_blocks, scalar_tail)) = try_vertical_append(
            content,
            frame,
            architecture,
            content_source,
        )? {
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
        let (image, vector_blocks, scalar_tail) = pad_mode(content, frame, architecture)?;
        record_vector_work(vector_blocks, scalar_tail);
        finish_pad(content, image, frame)
    }
}

fn finish_pad(
    source: &DynamicImage,
    mut output: DynamicImage,
    frame: PadFrame,
) -> Result<DynamicImage, PilError> {
    super::helpers::hidden_lanes::preserve_same_size_content(source, &mut output, frame)?;
    Ok(output)
}

fn record_vector_work(vector_blocks: u64, scalar_tail: u64) {
    if vector_blocks != 0 || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
}

fn try_vertical_append(
    image: &DynamicImage,
    frame: PadFrame,
    architecture: Architecture,
    content_source: PadContentSource,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    if architecture == Architecture::Default {
        return try_default_vertical_append(image, frame, content_source);
    }
    try_vector_vertical_append(image, frame, architecture, content_source)
}

fn try_default_vertical_append(
    image: &DynamicImage,
    frame: PadFrame,
    content_source: PadContentSource,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    use PadContentSource::{Input, Resized};
    match (content_source, image) {
        (Input | Resized, DynamicImage::ImageLuma8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::luma8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageLuma8(output), 0, 0)))
        }
        (Input, DynamicImage::ImageRgb8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::rgb8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageRgb8(output), 0, 0)))
        }
        (Input, DynamicImage::ImageHsv8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::hsv8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageHsv8(output), 0, 0)))
        }
        (Input, DynamicImage::ImageRgba8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::rgba8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageRgba8(output), 0, 0)))
        }
        (Resized, DynamicImage::ImageCmyk8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::cmyk8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageCmyk8(output), 0, 0)))
        }
        (Resized, DynamicImage::ImageRgbx8(image)) => {
            super::helpers::samples::pad_vertical_append(
                image,
                frame,
                super::rgbx8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|output| (DynamicImage::ImageRgbx8(output), 0, 0)))
        }
        _ => Ok(None),
    }
}

fn try_vector_vertical_append(
    image: &DynamicImage,
    frame: PadFrame,
    architecture: Architecture,
    content_source: PadContentSource,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    if content_source != PadContentSource::Input
        || !matches!(
            architecture,
            Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128
        )
    {
        return Ok(None);
    }
    match image {
        DynamicImage::ImageLuma8(image) => {
            super::helpers::native_vector::pad_vertical_append::<_, false>(
                image,
                frame,
                super::luma8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|(output, blocks, tail)| (DynamicImage::ImageLuma8(output), blocks, tail)))
        }
        DynamicImage::ImageRgb8(image) => {
            super::helpers::native_vector::pad_vertical_append::<_, false>(
                image,
                frame,
                super::rgb8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|(output, blocks, tail)| (DynamicImage::ImageRgb8(output), blocks, tail)))
        }
        DynamicImage::ImageRgba8(image) => {
            super::helpers::native_vector::pad_vertical_append::<_, true>(
                image,
                frame,
                super::rgba8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|(output, blocks, tail)| (DynamicImage::ImageRgba8(output), blocks, tail)))
        }
        DynamicImage::ImageCmyk8(image) => {
            super::helpers::native_vector::pad_vertical_append::<_, false>(
                image,
                frame,
                super::cmyk8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|(output, blocks, tail)| (DynamicImage::ImageCmyk8(output), blocks, tail)))
        }
        DynamicImage::ImageRgbx8(image) => {
            super::helpers::native_vector::pad_vertical_append::<_, false>(
                image,
                frame,
                super::rgbx8::default::pad_impl::fill(frame.color),
            )
            .map(|output| output.map(|(output, blocks, tail)| (DynamicImage::ImageRgbx8(output), blocks, tail)))
        }
        _ => Ok(None),
    }
}

fn try_direct_resize_into_canvas(
    image: &DynamicImage,
    frame: PadFrame,
    resize: Option<&crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'_>>,
    architecture: Architecture,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(resize) = resize else {
        return Ok(None);
    };
    if resize.selection().implementation.architecture != architecture {
        return Ok(None);
    }
    match image {
        DynamicImage::ImageLuma8(image) => {
            super::luma8::helpers::resize_window::resize_into_canvas(image, frame, resize)
                .map(|padded| padded.map(DynamicImage::ImageLuma8))
        }
        DynamicImage::ImageRgbx8(image) => {
            super::rgbx8::helpers::resize_window::resize_into_canvas(image, frame, resize)
                .map(|padded| padded.map(DynamicImage::ImageRgbx8))
        }
        _ => Ok(None),
    }
}

fn select(
    mode: Mode,
    facts: StrategyInputs,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match SUPPORT.for_mode(mode).assess(context, facts)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Pad preparation has unresolved input facts".into(),
        )),
    }
}

fn pad_mode(
    image: &DynamicImage,
    frame: PadFrame,
    architecture: Architecture,
) -> Result<(DynamicImage, u64, u64), PilError> {
    match image.mode() {
        Mode::One => super::one::ops::pad(image, frame, architecture),
        Mode::ImageLuma8 => super::luma8::ops::pad(image, frame, architecture),
        Mode::ImageLumaA8 => super::luma_a8::ops::pad(image, frame, architecture),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::pad(image, frame)
            .map(|image| (image, 0, 0)),
        Mode::ImagePalette8 => super::palette8::ops::pad(image, frame, architecture),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::pad(image, frame, architecture),
        Mode::ImageRgb8 => super::rgb8::ops::pad(image, frame, architecture),
        Mode::ImageRgba8 => super::rgba8::ops::pad(image, frame, architecture),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::pad(image, frame)
            .map(|image| (image, 0, 0)),
        Mode::ImageRgbx8 => super::rgbx8::ops::pad(image, frame, architecture),
        Mode::ImageCmyk8 => super::cmyk8::ops::pad(image, frame, architecture),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::pad(image, frame, architecture),
        Mode::ImageLab8 => super::lab8::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageHsv8 => super::hsv8::ops::pad(image, frame, architecture),
        Mode::ImageI32 => super::i32::ops::pad(image, frame, architecture),
        Mode::ImageF32 => super::f32::ops::pad(image, frame, architecture),
        Mode::ImageI16 => super::i16::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageI16L => super::i16l::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageI16B => super::i16b::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageI16N => super::i16n::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageLumaA16 => super::luma_a16::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageRgb16 => super::rgb16::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageRgba16 => super::rgba16::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageRgb32F => super::rgb32f::ops::pad(image, frame).map(|image| (image, 0, 0)),
        Mode::ImageRgba32F => super::rgba32f::ops::pad(image, frame).map(|image| (image, 0, 0)),
    }
}

pub(crate) fn prepare_pipeline<'a>(
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
        } => {
            (w, h, filter, color, None, *centering, None)
        }
        PipelineOp::PadTyped {
            w,
            h,
            filter,
            color,
            input,
            centering,
        } => {
            (w, h, filter, color, Some(*input), *centering, None)
        }
        PipelineOp::PadPositioned {
            w,
            h,
            filter,
            color,
            native_color,
            position,
        } => {
            (
                w,
                h,
                filter,
                color,
                *native_color,
                (0.5, 0.5),
                Some(*position),
            )
        }
        _ => return Err(PilError::ValueError("expected Pad op".into())),
    };
    let mode = inputs.mode();
    let facts = inputs.facts().with_pad(*w, *h, *filter, *color, centering);
    let selection = select(mode, facts, context)?;
    let mut frame = dimensions::build(
        (inputs.image.width(), inputs.image.height()),
        (*w, *h),
        centering,
        *color,
    )?;
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
    let resize = if native_identity {
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
    Ok(crate::compute::registry::PreparedOperation::from_pad(PreparedPad {
        inputs,
        frame,
        resize,
        selection,
    }))
}
