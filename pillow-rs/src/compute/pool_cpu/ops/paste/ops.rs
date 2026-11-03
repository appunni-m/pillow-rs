//! Paste operation support and exact-mode strategy orchestration.
//!
//! Exact L-mask NEON subkernel guards are declared beside each byte mode's
//! architecture support; the same-mode generic vector strategy remains the
//! fallback for other requests. TODO(pillow-rs#3ptb): carry the archived Rayon
//! clipping route into the later `pool_parallel` stage if retained, then cover
//! all mode-by-architecture paths and selection telemetry in CPU parity.

use std::sync::Arc;

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationInputs, OperationSupport, SecondaryInputFacts,
    StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) enum PreparedPaste<'a> {
    One(super::one::ops::Strategy<'a>),
    ImageLuma8(super::luma8::ops::Strategy<'a>),
    ImageLumaA8(super::luma_a8::ops::Strategy<'a>),
    ImageLaPremultiplied8(super::la_premultiplied8::ops::Strategy<'a>),
    ImagePalette8(super::palette8::ops::Strategy<'a>),
    ImagePaletteAlpha8(super::palette_alpha8::ops::Strategy<'a>),
    ImageRgb8(super::rgb8::ops::Strategy<'a>),
    ImageRgba8(super::rgba8::ops::Strategy<'a>),
    ImageRgbPremultiplied8(super::rgb_premultiplied8::ops::Strategy<'a>),
    ImageRgbx8(super::rgbx8::ops::Strategy<'a>),
    ImageCmyk8(super::cmyk8::ops::Strategy<'a>),
    ImageYCbCr8(super::ycbcr8::ops::Strategy<'a>),
    ImageLab8(super::lab8::ops::Strategy<'a>),
    ImageHsv8(super::hsv8::ops::Strategy<'a>),
    ImageI32(super::i32::ops::Strategy<'a>),
    ImageF32(super::f32::ops::Strategy<'a>),
    ImageI16(super::i16::ops::Strategy<'a>),
    ImageI16L(super::i16l::ops::Strategy<'a>),
    ImageI16B(super::i16b::ops::Strategy<'a>),
    ImageI16N(super::i16n::ops::Strategy<'a>),
    ImageLumaA16(super::luma_a16::ops::Strategy<'a>),
    ImageRgb16(super::rgb16::ops::Strategy<'a>),
    ImageRgba16(super::rgba16::ops::Strategy<'a>),
    ImageRgb32F(super::rgb32f::ops::Strategy<'a>),
    ImageRgba32F(super::rgba32f::ops::Strategy<'a>),
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
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
    }
}

/// Exhaustive Paste support, including core-only high-depth raster identities.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Paste, &[], classify_mode);

impl PreparedPaste<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::One(strategy) => &strategy.selection,
            Self::ImageLuma8(strategy) => &strategy.selection,
            Self::ImageLumaA8(strategy) => &strategy.selection,
            Self::ImageLaPremultiplied8(strategy) => &strategy.selection,
            Self::ImagePalette8(strategy) => &strategy.selection,
            Self::ImagePaletteAlpha8(strategy) => &strategy.selection,
            Self::ImageRgb8(strategy) => &strategy.selection,
            Self::ImageRgba8(strategy) => &strategy.selection,
            Self::ImageRgbPremultiplied8(strategy) => &strategy.selection,
            Self::ImageRgbx8(strategy) => &strategy.selection,
            Self::ImageCmyk8(strategy) => &strategy.selection,
            Self::ImageYCbCr8(strategy) => &strategy.selection,
            Self::ImageLab8(strategy) => &strategy.selection,
            Self::ImageHsv8(strategy) => &strategy.selection,
            Self::ImageI32(strategy) => &strategy.selection,
            Self::ImageF32(strategy) => &strategy.selection,
            Self::ImageI16(strategy) => &strategy.selection,
            Self::ImageI16L(strategy) => &strategy.selection,
            Self::ImageI16B(strategy) => &strategy.selection,
            Self::ImageI16N(strategy) => &strategy.selection,
            Self::ImageLumaA16(strategy) => &strategy.selection,
            Self::ImageRgb16(strategy) => &strategy.selection,
            Self::ImageRgba16(strategy) => &strategy.selection,
            Self::ImageRgb32F(strategy) => &strategy.selection,
            Self::ImageRgba32F(strategy) => &strategy.selection,
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::One(strategy) => super::one::ops::execute(strategy),
            Self::ImageLuma8(strategy) => super::luma8::ops::execute(strategy),
            Self::ImageLumaA8(strategy) => super::luma_a8::ops::execute(strategy),
            Self::ImageLaPremultiplied8(strategy) => super::la_premultiplied8::ops::execute(strategy),
            Self::ImagePalette8(strategy) => super::palette8::ops::execute(strategy),
            Self::ImagePaletteAlpha8(strategy) => super::palette_alpha8::ops::execute(strategy),
            Self::ImageRgb8(strategy) => super::rgb8::ops::execute(strategy),
            Self::ImageRgba8(strategy) => super::rgba8::ops::execute(strategy),
            Self::ImageRgbPremultiplied8(strategy) => super::rgb_premultiplied8::ops::execute(strategy),
            Self::ImageRgbx8(strategy) => super::rgbx8::ops::execute(strategy),
            Self::ImageCmyk8(strategy) => super::cmyk8::ops::execute(strategy),
            Self::ImageYCbCr8(strategy) => super::ycbcr8::ops::execute(strategy),
            Self::ImageLab8(strategy) => super::lab8::ops::execute(strategy),
            Self::ImageHsv8(strategy) => super::hsv8::ops::execute(strategy),
            Self::ImageI32(strategy) => super::i32::ops::execute(strategy),
            Self::ImageF32(strategy) => super::f32::ops::execute(strategy),
            Self::ImageI16(strategy) => super::i16::ops::execute(strategy),
            Self::ImageI16L(strategy) => super::i16l::ops::execute(strategy),
            Self::ImageI16B(strategy) => super::i16b::ops::execute(strategy),
            Self::ImageI16N(strategy) => super::i16n::ops::execute(strategy),
            Self::ImageLumaA16(strategy) => super::luma_a16::ops::execute(strategy),
            Self::ImageRgb16(strategy) => super::rgb16::ops::execute(strategy),
            Self::ImageRgba16(strategy) => super::rgba16::ops::execute(strategy),
            Self::ImageRgb32F(strategy) => super::rgb32f::ops::execute(strategy),
            Self::ImageRgba32F(strategy) => super::rgba32f::ops::execute(strategy),
        }
    }
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Paste { source, x, y, w, h, mask, mask_alpha } = op else {
        return Err(PilError::ValueError("expected Paste op".into()));
    };
    let width = u32::try_from(*w).map_err(|_| PilError::ValueError("images do not match".into()))?;
    let height = u32::try_from(*h).map_err(|_| PilError::ValueError("images do not match".into()))?;
    let mut operand_receipts = crate::compute::PipelineOperationTelemetryCapture::begin_preserving_current();
    let source = source.materialized_shared_for_ops()?;
    operand_receipts.capture();
    let mask = mask
        .as_deref()
        .map(crate::image::Image::materialized_shared_for_ops)
        .transpose()?;
    operand_receipts.capture();
    operand_receipts.restore();
    let source_facts = secondary_facts(source.as_ref());
    let mask_facts = mask.as_deref().map(secondary_facts);
    let facts = input
        .facts()
        .with_paste(
            source_facts,
            mask_facts,
            (i64::from(*x), i64::from(*y)),
            (width, height),
            *mask_alpha,
        );
    let mode_support = classify_mode(input.mode());
    let selection = match mode_support.assess(context, facts)? {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError("Paste preparation received unresolved input facts".into()));
        }
    };
    let position = (i64::from(*x), i64::from(*y));
    let region_size = (width, height);
    let prepared = prepare_mode(
        input.image,
        Arc::clone(&source),
        mask,
        *mask_alpha,
        position,
        region_size,
        selection,
    )?;
    Ok(crate::compute::registry::PreparedOperation::from_paste(prepared))
}

fn prepare_mode<'a>(
    image: &'a DynamicImage,
    source: Arc<DynamicImage>,
    mask: Option<Arc<DynamicImage>>,
    mask_alpha: bool,
    position: (i64, i64),
    region_size: (u32, u32),
    selection: StrategySelection,
) -> Result<PreparedPaste<'a>, PilError> {
    macro_rules! byte_mode {
        ($variant:ident, $module:ident) => {
            if let DynamicImage::$variant(image) = image {
                return super::$module::ops::prepare(
                    image, source, mask, mask_alpha, position, region_size, selection,
                )
                .map(PreparedPaste::$variant);
            }
        };
    }
    byte_mode!(One, one);
    byte_mode!(ImageLuma8, luma8);
    byte_mode!(ImageLumaA8, luma_a8);
    byte_mode!(ImageLaPremultiplied8, la_premultiplied8);
    byte_mode!(ImagePalette8, palette8);
    byte_mode!(ImagePaletteAlpha8, palette_alpha8);
    byte_mode!(ImageRgb8, rgb8);
    byte_mode!(ImageRgba8, rgba8);
    byte_mode!(ImageRgbPremultiplied8, rgb_premultiplied8);
    byte_mode!(ImageRgbx8, rgbx8);
    byte_mode!(ImageCmyk8, cmyk8);
    byte_mode!(ImageYCbCr8, ycbcr8);
    byte_mode!(ImageLab8, lab8);
    byte_mode!(ImageHsv8, hsv8);
    byte_mode!(ImageI32, i32);
    byte_mode!(ImageF32, f32);
    byte_mode!(ImageI16, i16);
    byte_mode!(ImageI16L, i16l);
    byte_mode!(ImageI16B, i16b);
    byte_mode!(ImageI16N, i16n);
    byte_mode!(ImageLumaA16, luma_a16);
    byte_mode!(ImageRgb16, rgb16);
    byte_mode!(ImageRgba16, rgba16);
    byte_mode!(ImageRgb32F, rgb32f);
    byte_mode!(ImageRgba32F, rgba32f);
    Err(PilError::InternalError("Paste mode strategy routing did not match image variant".into()))
}

fn secondary_facts(image: &DynamicImage) -> SecondaryInputFacts {
    SecondaryInputFacts {
        mode: image.mode(),
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
    }
}
