//! Transform operation routing and exhaustive exact-mode support.
//!
//! Public input classification, malformed-method errors, and error ordering
//! were compared with the pre-migration wrapper; those branches are unchanged.
//! The parity matrix must cover sampling, F, all four I;16 identities and byte
//! order behavior, P forced nearest versus filtered L, PA bicubic filtering,
//! concrete architecture selection/unavailability, mode declaration fallback,
//! and alpha/padding behavior. `fill.rs` handles palette index/color
//! normalization and errors. Keep mesh on Default: its archived adapter
//! groups scalar samples but has no vector kernel.
//! TODO(pillow-rs#3ptb): verify every mode/method/filter and architecture,
//! including unavailable-architecture and Default fallback receipts, plus
//! empty-mesh P palette retention with a non-default source palette.
//! `super::helpers::hidden_lanes` composes omitted physical bytes after the selected
//! mode/architecture kernel. Nearest sampling follows that architecture's
//! sampler; filtered LA/La/PA repeats the resulting first band into physical
//! lanes 1 and 2, while filtered three-band modes retain the output fill byte.

use crate::compute::pool_cpu::ops::transform::helpers::strategy::{
    self, Strategy,
};
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, RejectionKind, RequestFacts, StrategySelection,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, Mode};

pub(crate) const TRANSFORM_INPUTS: &[InputContract] = &[InputContract::TransformParameters];

#[derive(Debug, Clone, Copy)]
pub(crate) struct TransformRequest<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) method: &'a TransformMethod,
    pub(crate) data: &'a [f64],
    pub(crate) filter: ResampleFilter,
    pub(crate) fill: Option<(u8, u8, u8, u8)>,
    pub(crate) fill_is_none: bool,
}

impl TransformRequest<'_> {
    fn method_code(self) -> u8 {
        match self.method {
            TransformMethod::Affine => 0,
            TransformMethod::Perspective => 1,
            TransformMethod::Quad => 2,
            TransformMethod::Mesh => 3,
        }
    }

    pub(super) fn facts(self) -> RequestFacts {
        RequestFacts::Transform {
            method: self.method_code(),
            data_len: self.data.len(),
            filter: self.filter,
            coefficients_finite: self.data.iter().all(|value| value.is_finite()),
            output_width: self.width,
            output_height: self.height,
        }
    }

    fn from_pipeline(op: &PipelineOp) -> Result<TransformRequest<'_>, PilError> {
        let PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            fill,
            fill_is_none,
            palette_fill,
        } = op
        else {
            return Err(PilError::ValueError("expected Transform op".into()));
        };
        Ok(TransformRequest {
            width: *w,
            height: *h,
            method,
            data,
            filter: *filter,
            fill: palette_fill.map(|index| (index, 0, 0, 255)).or(*fill),
            fill_is_none: *fill_is_none,
        })
    }
}

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
        // NO_PILLOW_SUPPORT: these identities are Rust internal high-depth
        // storage modes without a Pillow Transform contract.
        Mode::ImageLumaA16 => rejected(mode, "Transform does not support internal 16-bit LA samples"),
        Mode::ImageRgb16 => rejected(mode, "Transform does not support internal 16-bit RGB samples"),
        Mode::ImageRgba16 => rejected(mode, "Transform does not support internal 16-bit RGBA samples"),
        Mode::ImageRgb32F => rejected(mode, "Transform does not support internal 32-bit float RGB samples"),
        Mode::ImageRgba32F => rejected(mode, "Transform does not support internal 32-bit float RGBA samples"),
    }
}

const fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::NotImplementedError,
    }
}

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Transform, TRANSFORM_INPUTS, classify_mode);

pub(crate) struct PreparedTransform<'a> {
    mode: Mode,
    strategy: Strategy<'a>,
}

impl PreparedTransform<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        strategy::selection(&self.strategy)
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let source = self.strategy.implementation.image;
        let request = self.strategy.implementation.request;
        let selection = *self.strategy.selection();
        let mut output = match self.mode {
            Mode::One => super::one::ops::execute(self.strategy),
            Mode::ImageLuma8 => super::luma8::ops::execute(self.strategy),
            Mode::ImageLumaA8 => super::luma_a8::ops::execute(self.strategy),
            Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::execute(self.strategy),
            Mode::ImagePalette8 => super::palette8::ops::execute(self.strategy),
            Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::execute(self.strategy),
            Mode::ImageRgb8 => super::rgb8::ops::execute(self.strategy),
            Mode::ImageRgba8 => super::rgba8::ops::execute(self.strategy),
            Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::execute(self.strategy),
            Mode::ImageRgbx8 => super::rgbx8::ops::execute(self.strategy),
            Mode::ImageCmyk8 => super::cmyk8::ops::execute(self.strategy),
            Mode::ImageYCbCr8 => super::ycbcr8::ops::execute(self.strategy),
            Mode::ImageLab8 => super::lab8::ops::execute(self.strategy),
            Mode::ImageHsv8 => super::hsv8::ops::execute(self.strategy),
            Mode::ImageI32 => super::i32::ops::execute(self.strategy),
            Mode::ImageF32 => super::f32::ops::execute(self.strategy),
            Mode::ImageI16 => super::i16::ops::execute(self.strategy),
            Mode::ImageI16L => super::i16l::ops::execute(self.strategy),
            Mode::ImageI16B => super::i16b::ops::execute(self.strategy),
            Mode::ImageI16N => super::i16n::ops::execute(self.strategy),
            mode => Err(PilError::InternalError(format!(
                "rejected Transform mode {mode:?} reached execution"
            ))),
        }?;
        super::helpers::hidden_lanes::finish(source, &mut output, request, &selection)?;
        Ok(output)
    }

    pub(crate) fn execute_into(self, output: &mut DynamicImage) -> Result<(), PilError> {
        let source = self.strategy.implementation.image;
        let request = self.strategy.implementation.request;
        let selection = *self.strategy.selection();
        match self.mode {
            Mode::One => super::one::ops::execute_into(self.strategy, output),
            Mode::ImageLuma8 => super::luma8::ops::execute_into(self.strategy, output),
            Mode::ImageLumaA8 => super::luma_a8::ops::execute_into(self.strategy, output),
            Mode::ImageLaPremultiplied8 => {
                super::la_premultiplied8::ops::execute_into(self.strategy, output)
            }
            Mode::ImagePalette8 => super::palette8::ops::execute_into(self.strategy, output),
            Mode::ImagePaletteAlpha8 => {
                super::palette_alpha8::ops::execute_into(self.strategy, output)
            }
            Mode::ImageRgb8 => super::rgb8::ops::execute_into(self.strategy, output),
            Mode::ImageRgba8 => super::rgba8::ops::execute_into(self.strategy, output),
            Mode::ImageRgbPremultiplied8 => {
                super::rgb_premultiplied8::ops::execute_into(self.strategy, output)
            }
            Mode::ImageRgbx8 => super::rgbx8::ops::execute_into(self.strategy, output),
            Mode::ImageCmyk8 => super::cmyk8::ops::execute_into(self.strategy, output),
            Mode::ImageYCbCr8 => super::ycbcr8::ops::execute_into(self.strategy, output),
            Mode::ImageLab8 => super::lab8::ops::execute_into(self.strategy, output),
            Mode::ImageHsv8 => super::hsv8::ops::execute_into(self.strategy, output),
            Mode::ImageI32 => super::i32::ops::execute_into(self.strategy, output),
            Mode::ImageF32 => super::f32::ops::execute_into(self.strategy, output),
            Mode::ImageI16 => super::i16::ops::execute_into(self.strategy, output),
            Mode::ImageI16L => super::i16l::ops::execute_into(self.strategy, output),
            Mode::ImageI16B => super::i16b::ops::execute_into(self.strategy, output),
            Mode::ImageI16N => super::i16n::ops::execute_into(self.strategy, output),
            mode => Err(PilError::InternalError(format!(
                "rejected Transform mode {mode:?} reached direct destination execution"
            ))),
        }?;
        super::helpers::hidden_lanes::finish(source, output, request, &selection)
    }

    pub(crate) fn apply_mesh_record(
        &self,
        output: &mut DynamicImage,
        box_coordinates: (i64, i64, i64, i64),
        coefficients: [f64; 8],
        source_scratch: &mut Vec<u8>,
        i16_output_plane: &mut Option<Vec<u8>>,
    ) -> Result<(), PilError> {
        let implementation = &self.strategy.implementation;
        let request = implementation.request;
        let selection = self.selection();
        if !matches!(request.method, TransformMethod::Mesh)
            || implementation.image.mode() != self.mode
            || output.mode() != self.mode
        {
            return Err(PilError::InternalError(
                "Transform MESH record mode or request mismatch".into(),
            ));
        }
        if selection.implementation.architecture != Architecture::Default {
            return Err(PilError::InternalError(
                "Transform MESH selected an unavailable architecture kernel".into(),
            ));
        }
        let input = implementation.image;
        match self.mode {
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
            | Mode::ImageHsv8 => apply_byte_mesh_record(
                self.mode,
                input,
                output,
                request,
                box_coordinates,
                coefficients,
                source_scratch,
            ),
            Mode::ImageI32 => super::i32::default::transform_impl::apply_mesh_record(
                input, output, request, box_coordinates, coefficients,
            ),
            Mode::ImageF32 => super::f32::default::transform_impl::apply_mesh_record(
                input, output, request, box_coordinates, coefficients,
            ),
            Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N => apply_i16_mesh_record(
                self.mode,
                input,
                output,
                request,
                box_coordinates,
                coefficients,
                source_scratch,
                i16_output_plane,
            ),
            mode => Err(PilError::InternalError(format!(
                "rejected Transform MESH mode {mode:?} reached execution"
            ))),
        }
    }
}

fn apply_byte_mesh_record(
    mode: Mode,
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
) -> Result<(), PilError> {
    match mode {
        Mode::One => super::one::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageLuma8 => super::luma8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageLumaA8 => super::luma_a8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::default::transform_impl::apply_mesh_record(
                input, output, request, box_coordinates, coefficients, source_scratch,
            )
        }
        Mode::ImagePalette8 => super::palette8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImagePaletteAlpha8 => {
            super::palette_alpha8::default::transform_impl::apply_mesh_record(
                input, output, request, box_coordinates, coefficients, source_scratch,
            )
        }
        Mode::ImageRgb8 => super::rgb8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageRgba8 => super::rgba8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::default::transform_impl::apply_mesh_record(
                input, output, request, box_coordinates, coefficients, source_scratch,
            )
        }
        Mode::ImageRgbx8 => super::rgbx8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageCmyk8 => super::cmyk8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageYCbCr8 => super::ycbcr8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageLab8 => super::lab8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        Mode::ImageHsv8 => super::hsv8::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch,
        ),
        _ => Err(PilError::InternalError(
            "non-byte mode reached Transform MESH byte router".into(),
        )),
    }
}

fn apply_i16_mesh_record(
    mode: Mode,
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
    output_plane: &mut Option<Vec<u8>>,
) -> Result<(), PilError> {
    match mode {
        Mode::ImageI16 => super::i16::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch, output_plane,
        ),
        Mode::ImageI16L => super::i16l::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch, output_plane,
        ),
        Mode::ImageI16B => super::i16b::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch, output_plane,
        ),
        Mode::ImageI16N => super::i16n::default::transform_impl::apply_mesh_record(
            input, output, request, box_coordinates, coefficients, source_scratch, output_plane,
        ),
        _ => Err(PilError::InternalError(
            "non-I;16 mode reached Transform MESH I;16 router".into(),
        )),
    }
}

fn prepare_mode<'a>(
    image: &'a DynamicImage,
    request: TransformRequest<'a>,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    match image {
        DynamicImage::One(_) => super::one::ops::prepare(image, request, context),
        DynamicImage::ImageLuma8(_) => super::luma8::ops::prepare(image, request, context),
        DynamicImage::ImageLumaA8(_) => super::luma_a8::ops::prepare(image, request, context),
        DynamicImage::ImageLaPremultiplied8(_) => {
            super::la_premultiplied8::ops::prepare(image, request, context)
        }
        DynamicImage::ImagePalette8(_) => super::palette8::ops::prepare(image, request, context),
        DynamicImage::ImagePaletteAlpha8(_) => {
            super::palette_alpha8::ops::prepare(image, request, context)
        }
        DynamicImage::ImageRgb8(_) => super::rgb8::ops::prepare(image, request, context),
        DynamicImage::ImageRgba8(_) => super::rgba8::ops::prepare(image, request, context),
        DynamicImage::ImageRgbPremultiplied8(_) => {
            super::rgb_premultiplied8::ops::prepare(image, request, context)
        }
        DynamicImage::ImageRgbx8(_) => super::rgbx8::ops::prepare(image, request, context),
        DynamicImage::ImageCmyk8(_) => super::cmyk8::ops::prepare(image, request, context),
        DynamicImage::ImageYCbCr8(_) => super::ycbcr8::ops::prepare(image, request, context),
        DynamicImage::ImageLab8(_) => super::lab8::ops::prepare(image, request, context),
        DynamicImage::ImageHsv8(_) => super::hsv8::ops::prepare(image, request, context),
        DynamicImage::ImageI32(_) => super::i32::ops::prepare(image, request, context),
        DynamicImage::ImageF32(_) => super::f32::ops::prepare(image, request, context),
        DynamicImage::ImageI16(_) => super::i16::ops::prepare(image, request, context),
        DynamicImage::ImageI16L(_) => super::i16l::ops::prepare(image, request, context),
        DynamicImage::ImageI16B(_) => super::i16b::ops::prepare(image, request, context),
        DynamicImage::ImageI16N(_) => super::i16n::ops::prepare(image, request, context),
        image => match SUPPORT.for_mode(image.mode()) {
            ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
            ModeSupport::Supported { .. } => Err(PilError::InternalError(
                "Transform support declaration has no matching mode router".into(),
            )),
        },
    }
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    request: TransformRequest<'a>,
    context: &ExecutionContext,
) -> Result<PreparedTransform<'a>, PilError> {
    let mode = image.mode();
    let strategy = prepare_mode(image, request, context)?;
    Ok(PreparedTransform { mode, strategy })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let request = TransformRequest::from_pipeline(op)?;
    prepare(input.image, request, context)
        .map(crate::compute::registry::PreparedOperation::from_transform)
}

pub(crate) fn finish_mesh_records(
    mode: Mode,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    output_plane: Option<&[u8]>,
) -> Result<(), PilError> {
    match mode {
        Mode::ImageI16 => super::i16::default::transform_impl::finish_mesh(
            output, request, output_plane,
        ),
        Mode::ImageI16L => super::i16l::default::transform_impl::finish_mesh(
            output, request, output_plane,
        ),
        Mode::ImageI16B => super::i16b::default::transform_impl::finish_mesh(
            output, request, output_plane,
        ),
        Mode::ImageI16N => super::i16n::default::transform_impl::finish_mesh(
            output, request, output_plane,
        ),
        _ => Ok(()),
    }
}
