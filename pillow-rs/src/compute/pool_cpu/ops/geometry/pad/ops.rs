//! Pad support declarations and prepared exact-mode Resize composition.
// TODO(pillow-rs#3ptb): let full-width Pad windows compose the newly migrated
// byte-mode vector Resize strategies, then reconcile and retire the old SIMD
// Pad route after its callers use this prepared CPU context.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport,
    OutputModeRule, StrategyInputs, StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

use super::helpers::dimensions::{self, PadFrame};

pub(super) const INPUTS: &[InputContract] = &[InputContract::PadParameters];
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
    INPUTS,
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
        let architecture = self.selection.implementation.architecture;
        if let Some(padded) = try_direct_resize_into_canvas(
            self.inputs.image,
            self.frame,
            self.resize.as_ref(),
            architecture,
        )? {
            if let Some(resize) = self.resize.as_ref() {
                crate::compute::record_pipeline_operation_architecture(
                    resize.selection().implementation.architecture,
                );
            }
            return Ok(padded);
        }
        let resized = match self.resize {
            Some(resize) => {
                crate::compute::record_pipeline_operation_architecture(
                    resize.selection().implementation.architecture,
                );
                Some(resize.execute()?)
            }
            None => None,
        };
        let image = resized.as_ref().unwrap_or(self.inputs.image);
        if architecture == Architecture::Default
            && self.frame.content_width == self.frame.width
            && self.frame.content_height == self.frame.height
        {
            return Ok(image.clone());
        }
        let (image, vector_blocks, scalar_tail) = pad_mode(image, self.frame, architecture)?;
        if vector_blocks != 0 || scalar_tail != 0 {
            crate::compute::record_pipeline_operation_path("vector");
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        Ok(image)
    }
}

fn try_direct_resize_into_canvas(
    image: &DynamicImage,
    frame: PadFrame,
    resize: Option<&crate::compute::pool_cpu::ops::resize::ops::PreparedResize<'_>>,
    architecture: Architecture,
) -> Result<Option<DynamicImage>, PilError> {
    if architecture != Architecture::Default {
        return Ok(None);
    }
    let Some(resize) = resize else {
        return Ok(None);
    };
    match image {
        DynamicImage::ImageLuma8(image) => {
            super::luma8::default::pad_impl::resize_into_canvas(image, frame, resize)
                .map(|padded| padded.map(DynamicImage::ImageLuma8))
        }
        DynamicImage::ImageRgbx8(image) => {
            super::rgbx8::default::pad_impl::resize_into_canvas(image, frame, resize)
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
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::pad(image, frame, architecture),
        Mode::ImagePalette8 => super::palette8::ops::pad(image, frame, architecture),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::pad(image, frame, architecture),
        Mode::ImageRgb8 => super::rgb8::ops::pad(image, frame, architecture),
        Mode::ImageRgba8 => super::rgba8::ops::pad(image, frame, architecture),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::pad(image, frame, architecture),
        Mode::ImageRgbx8 => super::rgbx8::ops::pad(image, frame, architecture),
        Mode::ImageCmyk8 => super::cmyk8::ops::pad(image, frame, architecture),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::pad(image, frame, architecture),
        Mode::ImageLab8 => super::lab8::ops::pad(image, frame, architecture),
        Mode::ImageHsv8 => super::hsv8::ops::pad(image, frame, architecture),
        Mode::ImageI32 => super::i32::ops::pad(image, frame, architecture),
        Mode::ImageF32 => super::f32::ops::pad(image, frame, architecture),
        Mode::ImageI16 => super::i16::ops::pad(image, frame, architecture),
        Mode::ImageI16L => super::i16l::ops::pad(image, frame, architecture),
        Mode::ImageI16B => super::i16b::ops::pad(image, frame, architecture),
        Mode::ImageI16N => super::i16n::ops::pad(image, frame, architecture),
        Mode::ImageLumaA16 => super::luma_a16::ops::pad(image, frame, architecture),
        Mode::ImageRgb16 => super::rgb16::ops::pad(image, frame, architecture),
        Mode::ImageRgba16 => super::rgba16::ops::pad(image, frame, architecture),
        Mode::ImageRgb32F => super::rgb32f::ops::pad(image, frame, architecture),
        Mode::ImageRgba32F => super::rgba32f::ops::pad(image, frame, architecture),
    }
}

pub(crate) fn prepare_pipeline<'a>(
    inputs: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Pad { w, h, filter, color, centering } = op else {
        return Err(PilError::ValueError("expected Pad op".into()));
    };
    let mode = inputs.mode();
    let facts = inputs.facts().with_pad(*w, *h, *filter, *color, *centering);
    let selection = select(mode, facts, context)?;
    let frame = dimensions::build(
        (inputs.image.width(), inputs.image.height()),
        (*w, *h),
        *centering,
        *color,
    )?;
    let filter = if mode == Mode::ImagePalette8 {
        ResampleFilter::Nearest
    } else {
        *filter
    };
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
            filter,
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
