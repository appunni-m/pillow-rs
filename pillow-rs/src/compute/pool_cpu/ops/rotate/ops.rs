//! Rotate entry point and exhaustive exact-mode strategy composition.
//!
//! Public rotation calls enqueue a typed `PipelineOp::Rotate`; the operation
//! registry prepares this family with the routed CPU context. The Python
//! facade returns an identity copy and routes half-turn plus eligible quarter-
//! turn calls through typed Transpose operations. Other base-image calls stage
//! fill/filter preparation, load at Pillow's route-specific boundary, then
//! attach host-computed affine coefficients.
//! Subclass overrides retain public `transform`/`convert` dispatch. Direct Rust
//! and JavaScript callers enqueue the same typed Rotate operation. All 25
//! `DynamicImage` variants dispatch to an exact-mode owner, which selects from
//! its `SUPPORT` declaration and returns that exact raster identity. Byte-backed
//! exact modes declare native-byte nearest strategies; byte modes also declare
//! their archived bilinear kernel where supported. Three-byte modes retain the
//! contiguous-run copy fast path. Empty-source requests use each exact mode's
//! Default path; every mode's Default strategy is also its architecture fallback.
//! Exact identity and discrete 90/180/270-degree paths remap retained physical
//! lanes with their pixels. Affine nearest samples retained lanes from the
//! selected Default/vector nearest geometry; filtered LA-family output repeats
//! luma into the omitted bytes, while filtered RGB-family output preserves the
//! configured physical padding fill. Runtime lane parity remains pending.
//! TODO(pillow-rs#3ptb): any future I;16* Architecture strategy must sample
//! typed u16 values with Pillow's floating Geometry.c affine and exact fill
//! semantics. Default output parity covers all four exact identities, three
//! filters, and arbitrary/custom transforms. Do not port the archived raw-byte
//! SIMD nearest kernel: its signed 16.16 stepping and byte-plane sampling are
//! not equivalent. Keep these modes Default-only with same-mode fallback until
//! a typed strategy is implemented and parity-verified.
//! Core-only mode support is marked `NO_PILLOW_SUPPORT` beside its
//! implementation.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, RequestFacts,
    StrategySelection,
};
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
    pub(crate) affine: Option<[f64; 6]>,
    pub(crate) output_size: Option<(u32, u32)>,
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
            affine_bits: self.affine.map(|values| values.map(f64::to_bits)),
            output_size: self.output_size,
            filter: filter_code(self.filter),
            nearest: self.nearest,
        }
    }

    fn from_pipeline(op: &PipelineOp) -> Result<Self, PilError> {
        let PipelineOp::Rotate {
            angle,
            expand,
            fill,
            center,
            translate,
            affine,
            output_size,
            filter,
            nearest,
        } = op
        else {
            return Err(PilError::ValueError("expected Rotate op".into()));
        };
        Ok(Self {
            angle: *angle,
            expand: *expand,
            fill: *fill,
            center: *center,
            translate: *translate,
            affine: *affine,
            output_size: *output_size,
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
        enum RotateStrategy<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        pub(crate) struct PreparedRotate<'a> {
            image: &'a DynamicImage,
            request: RotateRequest,
            strategy: RotateStrategy<'a>,
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::SUPPORT,)+ }
        }

        fn prepare_mode<'a>(
            image: &'a DynamicImage,
            request: RotateRequest,
            context: &ExecutionContext,
        ) -> Result<RotateStrategy<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, request, context)
                    .map(RotateStrategy::$mode),)+
            }
        }

        impl PreparedRotate<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match &self.strategy { $(RotateStrategy::$mode(plan) => plan.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                let selection = *self.selection();
                let Self { image, request, strategy } = self;
                let mut output = match strategy {
                    $(RotateStrategy::$mode(plan) => super::$module::ops::execute(plan),)+
                }?;
                super::helpers::hidden_lanes::finish(image, &mut output, request, &selection)?;
                Ok(output)
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

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<PreparedRotate<'a>, PilError> {
    let strategy = prepare_mode(image, request, context)?;
    Ok(PreparedRotate {
        image,
        request,
        strategy,
    })
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
