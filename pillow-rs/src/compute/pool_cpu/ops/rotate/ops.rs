//! Rotate entry point and exhaustive exact-mode strategy composition.
//!
//! TODO(pillow-rs#3ptb): route pipeline, eager, and compatibility callers
//! through the injected CPU context. All byte-backed exact modes declare
//! native-byte nearest strategies; byte modes also declare their archived
//! bilinear kernel where supported. Three-byte modes retain the contiguous-run
//! copy fast path. Each mode's Default strategy remains its same-mode fallback.
//! Core-only mode support is valid when marked `NO_PILLOW_SUPPORT` beside its
//! implementation.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, PaletteKnowledge,
    RequestFacts, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, Mode};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RotateRequest {
    pub(crate) angle: f64,
    pub(crate) expand: bool,
    pub(crate) fill: Option<(u8, u8, u8, u8)>,
    pub(crate) center: Option<(f64, f64)>,
    pub(crate) translate: Option<(f64, f64)>,
    pub(crate) filter: ResampleFilter,
    pub(crate) nearest: bool,
}

impl RotateRequest {
    pub(crate) fn facts(self) -> RequestFacts {
        RequestFacts::Rotate {
            angle_bits: self.angle.to_bits(),
            expand: self.expand,
            fill: self.fill,
            center_bits: self.center.map(|(x, y)| (x.to_bits(), y.to_bits())),
            translate_bits: self.translate.map(|(x, y)| (x.to_bits(), y.to_bits())),
            filter: filter_code(self.filter),
            nearest: self.nearest,
        }
    }

    fn from_pipeline(op: &PipelineOp) -> Result<Self, PilError> {
        let PipelineOp::Rotate { angle, expand, fill, center, translate, filter, nearest } = op else {
            return Err(PilError::ValueError("expected Rotate op".into()));
        };
        Ok(Self {
            angle: *angle,
            expand: *expand,
            fill: *fill,
            center: *center,
            translate: *translate,
            filter: *filter,
            nearest: *nearest,
        })
    }
}

fn filter_code(filter: ResampleFilter) -> u8 {
    match filter {
        ResampleFilter::Nearest => 0,
        ResampleFilter::Bilinear => 1,
        ResampleFilter::Bicubic => 2,
        ResampleFilter::Lanczos | ResampleFilter::Box | ResampleFilter::Hamming => 3,
    }
}

pub(crate) const ROTATE_INPUTS: &[InputContract] = &[InputContract::Rotate];

macro_rules! declare_rotate_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedRotate<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::SUPPORT,)+ }
        }

        fn prepare_mode<'a>(
            image: &'a DynamicImage,
            request: RotateRequest,
            context: &ExecutionContext,
        ) -> Result<PreparedRotate<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, request, context)
                    .map(PreparedRotate::$mode),)+
            }
        }

        impl PreparedRotate<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(plan) => plan.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(plan) => super::$module::ops::execute(plan),)+ }
            }
        }
    };
}

declare_rotate_modes! {
    One => one,
    ImageLuma8 => luma8,
    ImageLumaA8 => luma_a8,
    ImageLaPremultiplied8 => la_premultiplied8,
    ImagePalette8 => palette8,
    ImagePaletteAlpha8 => palette_alpha8,
    ImageRgb8 => rgb8,
    ImageRgba8 => rgba8,
    ImageRgbPremultiplied8 => rgb_premultiplied8,
    ImageRgbx8 => rgbx8,
    ImageCmyk8 => cmyk8,
    ImageYCbCr8 => ycbcr8,
    ImageLab8 => lab8,
    ImageHsv8 => hsv8,
    ImageI32 => i32,
    ImageF32 => f32,
    ImageI16 => i16,
    ImageI16L => i16l,
    ImageI16B => i16b,
    ImageI16N => i16n,
    ImageLumaA16 => luma_a16,
    ImageRgb16 => rgb16,
    ImageRgba16 => rgba16,
    ImageRgb32F => rgb32f,
    ImageRgba32F => rgba32f,
}

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Rotate, ROTATE_INPUTS, classify_mode);

pub(crate) fn assess(
    mode: Mode,
    size: (u32, u32),
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let input = StrategyInputs {
        width: size.0,
        height: size.1,
        byte_len: None,
        palette: PaletteKnowledge::Absent,
        request: request.facts(),
    };
    SUPPORT.for_mode(mode).assess(context, input)
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<PreparedRotate<'a>, PilError> {
    prepare_mode(image, request, context)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    prepare(image, request, context)?.execute()
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let request = RotateRequest::from_pipeline(op)?;
    prepare(input.image, request, context)
        .map(crate::compute::registry::PreparedOperation::from_rotate)
}
