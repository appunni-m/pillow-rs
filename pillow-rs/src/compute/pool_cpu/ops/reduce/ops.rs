//! Reduce orchestration and exhaustive mode support.
//!
//! TODO(pillow-rs#3ptb): share I/F helpers and RGB sparse-row facts with
//! Thumbnail when that family is migrated, and move archived Rayon scheduling
//! into pool_parallel after CPU is complete. L, LA, RGB, RGBA, CMYK, YCbCr,
//! and HSV declare AVX2 plus portable SSE2, NEON, and wasm SIMD strategies
//! where those kernels are built for the target. Other modes retain their
//! same-mode Default strategy.

// NO_PILLOW_SUPPORT: typed I;16 reductions extend Pillow's current Reduce contract.
// NO_PILLOW_SUPPORT: internal high-depth modes use native typed strategies.

use crate::compute::{
    ExecutionContext,
    registry::support::{
        OperationId, OperationInputs, OperationSupport, PaletteKnowledge, RequestFacts,
        StrategyInputs, SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) enum PreparedReduce<'a> {
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

fn classify_mode(mode: Mode) -> crate::compute::registry::support::ModeSupport {
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

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Reduce, &[], classify_mode);

pub(crate) fn assess(
    mode: Mode,
    size: (u32, u32),
    factors: (u32, u32),
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let input = StrategyInputs {
        width: size.0,
        height: size.1,
        byte_len: None,
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::Reduce {
            x_factor: factors.0,
            y_factor: factors.1,
        },
    };
    SUPPORT.for_mode(mode).assess(ctx, input)
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    factors: (u32, u32),
    ctx: &ExecutionContext,
) -> Result<PreparedReduce<'a>, PilError> {
    let x_factor = factors.0.max(1);
    let y_factor = factors.1.max(1);
    let facts = input.facts().with_reduce_factors(x_factor, y_factor);
    match input.image {
        DynamicImage::One(image) => super::one::ops::prepare(image, facts, ctx).map(PreparedReduce::One),
        DynamicImage::ImageLuma8(image) => super::luma8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageLuma8),
        DynamicImage::ImageLumaA8(image) => super::luma_a8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageLumaA8),
        DynamicImage::ImageLaPremultiplied8(image) => super::la_premultiplied8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageLaPremultiplied8),
        DynamicImage::ImagePalette8(image) => super::palette8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImagePalette8),
        DynamicImage::ImagePaletteAlpha8(image) => super::palette_alpha8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImagePaletteAlpha8),
        DynamicImage::ImageRgb8(image) => super::rgb8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgb8),
        DynamicImage::ImageRgba8(image) => super::rgba8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgba8),
        DynamicImage::ImageRgbPremultiplied8(image) => super::rgb_premultiplied8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgbPremultiplied8),
        DynamicImage::ImageRgbx8(image) => super::rgbx8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgbx8),
        DynamicImage::ImageCmyk8(image) => super::cmyk8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageCmyk8),
        DynamicImage::ImageYCbCr8(image) => super::ycbcr8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageYCbCr8),
        DynamicImage::ImageLab8(image) => super::lab8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageLab8),
        DynamicImage::ImageHsv8(image) => super::hsv8::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageHsv8),
        DynamicImage::ImageI32(image) => super::i32::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageI32),
        DynamicImage::ImageF32(image) => super::f32::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageF32),
        DynamicImage::ImageI16(image) => super::i16::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageI16),
        DynamicImage::ImageI16L(image) => super::i16l::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageI16L),
        DynamicImage::ImageI16B(image) => super::i16b::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageI16B),
        DynamicImage::ImageI16N(image) => super::i16n::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageI16N),
        DynamicImage::ImageLumaA16(image) => super::luma_a16::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageLumaA16),
        DynamicImage::ImageRgb16(image) => super::rgb16::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgb16),
        DynamicImage::ImageRgba16(image) => super::rgba16::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgba16),
        DynamicImage::ImageRgb32F(image) => super::rgb32f::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgb32F),
        DynamicImage::ImageRgba32F(image) => super::rgba32f::ops::prepare(image, facts, ctx).map(PreparedReduce::ImageRgba32F),
    }
}

impl PreparedReduce<'_> {
    pub(crate) fn selection(
        &self,
    ) -> &crate::compute::registry::support::StrategySelection {
        match self {
            Self::One(strategy) => strategy.selection(),
            Self::ImageLuma8(strategy) => strategy.selection(),
            Self::ImageLumaA8(strategy) => strategy.selection(),
            Self::ImageLaPremultiplied8(strategy) => strategy.selection(),
            Self::ImagePalette8(strategy) => strategy.selection(),
            Self::ImagePaletteAlpha8(strategy) => strategy.selection(),
            Self::ImageRgb8(strategy) => strategy.selection(),
            Self::ImageRgba8(strategy) => strategy.selection(),
            Self::ImageRgbPremultiplied8(strategy) => strategy.selection(),
            Self::ImageRgbx8(strategy) => strategy.selection(),
            Self::ImageCmyk8(strategy) => strategy.selection(),
            Self::ImageYCbCr8(strategy) => strategy.selection(),
            Self::ImageLab8(strategy) => strategy.selection(),
            Self::ImageHsv8(strategy) => strategy.selection(),
            Self::ImageI32(strategy) => strategy.selection(),
            Self::ImageF32(strategy) => strategy.selection(),
            Self::ImageI16(strategy) => strategy.selection(),
            Self::ImageI16L(strategy) => strategy.selection(),
            Self::ImageI16B(strategy) => strategy.selection(),
            Self::ImageI16N(strategy) => strategy.selection(),
            Self::ImageLumaA16(strategy) => strategy.selection(),
            Self::ImageRgb16(strategy) => strategy.selection(),
            Self::ImageRgba16(strategy) => strategy.selection(),
            Self::ImageRgb32F(strategy) => strategy.selection(),
            Self::ImageRgba32F(strategy) => strategy.selection(),
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
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Reduce { x_factor, y_factor } = op else {
        return Err(PilError::ValueError("expected Reduce op".into()));
    };
    prepare(input, (*x_factor, *y_factor), ctx)
        .map(crate::compute::registry::PreparedOperation::from_reduce)
}
