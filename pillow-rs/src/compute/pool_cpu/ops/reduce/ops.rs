//! Reduce orchestration and exhaustive mode support.
//!
//! Prepared execution composes Pillow's physical padding after each exact
//! mode/architecture reducer, so every native route retains one lane contract.
//! Non-fused Thumbnail reduction reuses this prepared Reduce owner, including
//! the I/F helpers; RGB PutPixel/Thumbnail fusion consumes the shared sparse-
//! row reducer. TODO(pillow-rs#3ptb): move archived Rayon scheduling into
//! pool_parallel after CPU is complete. L, LA, RGB, RGBA, CMYK, YCbCr,
//! and HSV declare native-byte AVX2 plus portable SSE2, NEON, and wasm SIMD
//! strategies where those kernels are built for the target. F32 declares its
//! portable eight-lane even 2x2 kernel as AVX or SSE2 when compiled for those
//! features, with NEON and wasm SIMD128 choices where built. Other requests
//! and modes retain their same-mode Default strategy.
//! TODO(pillow-rs#3ptb): verify native I;16 values, per-mode output identity,
//! architecture selection/fallback, and straight-alpha LA/RGBA premultiply →
//! reduce → unpremultiply rounding across the frozen CPU matrix.

// NO_PILLOW_SUPPORT: Pillow 13 rejects I16 storage in Reduce; these routes preserve native u16 samples.
// NO_PILLOW_SUPPORT: internal high-depth modes use native typed strategies.

use crate::compute::{
    ExecutionContext,
    registry::support::{OperationId, OperationInputs, OperationSupport},
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) struct PreparedReduce<'a> {
    image: &'a DynamicImage,
    factors: (u32, u32),
    strategy: ReduceStrategy<'a>,
}

enum ReduceStrategy<'a> {
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    factors: (u32, u32),
    ctx: &ExecutionContext,
) -> Result<PreparedReduce<'a>, PilError> {
    let (x_factor, y_factor) = factors;
    let facts = input.facts().with_reduce_factors(x_factor, y_factor);
    let strategy = match input.image {
        DynamicImage::One(image) => {
            super::one::ops::prepare(image, facts, ctx).map(ReduceStrategy::One)
        }
        DynamicImage::ImageLuma8(image) => {
            super::luma8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageLuma8)
        }
        DynamicImage::ImageLumaA8(image) => {
            super::luma_a8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageLumaA8)
        }
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::la_premultiplied8::ops::prepare(image, facts, ctx)
                .map(ReduceStrategy::ImageLaPremultiplied8)
        }
        DynamicImage::ImagePalette8(image) => {
            super::palette8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImagePalette8)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::palette_alpha8::ops::prepare(image, facts, ctx)
                .map(ReduceStrategy::ImagePaletteAlpha8)
        }
        DynamicImage::ImageRgb8(image) => {
            super::rgb8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgb8)
        }
        DynamicImage::ImageRgba8(image) => {
            super::rgba8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgba8)
        }
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::rgb_premultiplied8::ops::prepare(image, facts, ctx)
                .map(ReduceStrategy::ImageRgbPremultiplied8)
        }
        DynamicImage::ImageRgbx8(image) => {
            super::rgbx8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgbx8)
        }
        DynamicImage::ImageCmyk8(image) => {
            super::cmyk8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageCmyk8)
        }
        DynamicImage::ImageYCbCr8(image) => {
            super::ycbcr8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageYCbCr8)
        }
        DynamicImage::ImageLab8(image) => {
            super::lab8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageLab8)
        }
        DynamicImage::ImageHsv8(image) => {
            super::hsv8::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageHsv8)
        }
        DynamicImage::ImageI32(image) => {
            super::i32::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageI32)
        }
        DynamicImage::ImageF32(image) => {
            super::f32::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageF32)
        }
        DynamicImage::ImageI16(image) => {
            super::i16::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageI16)
        }
        DynamicImage::ImageI16L(image) => {
            super::i16l::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageI16L)
        }
        DynamicImage::ImageI16B(image) => {
            super::i16b::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageI16B)
        }
        DynamicImage::ImageI16N(image) => {
            super::i16n::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageI16N)
        }
        DynamicImage::ImageLumaA16(image) => {
            super::luma_a16::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageLumaA16)
        }
        DynamicImage::ImageRgb16(image) => {
            super::rgb16::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgb16)
        }
        DynamicImage::ImageRgba16(image) => {
            super::rgba16::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgba16)
        }
        DynamicImage::ImageRgb32F(image) => {
            super::rgb32f::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgb32F)
        }
        DynamicImage::ImageRgba32F(image) => {
            super::rgba32f::ops::prepare(image, facts, ctx).map(ReduceStrategy::ImageRgba32F)
        }
    }?;
    Ok(PreparedReduce {
        image: input.image,
        factors: (x_factor, y_factor),
        strategy,
    })
}

impl PreparedReduce<'_> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &crate::compute::registry::support::StrategySelection {
        match &self.strategy {
            ReduceStrategy::One(strategy) => strategy.selection(),
            ReduceStrategy::ImageLuma8(strategy) => strategy.selection(),
            ReduceStrategy::ImageLumaA8(strategy) => strategy.selection(),
            ReduceStrategy::ImageLaPremultiplied8(strategy) => strategy.selection(),
            ReduceStrategy::ImagePalette8(strategy) => strategy.selection(),
            ReduceStrategy::ImagePaletteAlpha8(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgb8(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgba8(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgbPremultiplied8(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgbx8(strategy) => strategy.selection(),
            ReduceStrategy::ImageCmyk8(strategy) => strategy.selection(),
            ReduceStrategy::ImageYCbCr8(strategy) => strategy.selection(),
            ReduceStrategy::ImageLab8(strategy) => strategy.selection(),
            ReduceStrategy::ImageHsv8(strategy) => strategy.selection(),
            ReduceStrategy::ImageI32(strategy) => strategy.selection(),
            ReduceStrategy::ImageF32(strategy) => strategy.selection(),
            ReduceStrategy::ImageI16(strategy) => strategy.selection(),
            ReduceStrategy::ImageI16L(strategy) => strategy.selection(),
            ReduceStrategy::ImageI16B(strategy) => strategy.selection(),
            ReduceStrategy::ImageI16N(strategy) => strategy.selection(),
            ReduceStrategy::ImageLumaA16(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgb16(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgba16(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgb32F(strategy) => strategy.selection(),
            ReduceStrategy::ImageRgba32F(strategy) => strategy.selection(),
        }
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let Self {
            image,
            factors,
            strategy,
        } = self;
        let mut output = match strategy {
            ReduceStrategy::One(strategy) => super::one::ops::execute(strategy),
            ReduceStrategy::ImageLuma8(strategy) => super::luma8::ops::execute(strategy),
            ReduceStrategy::ImageLumaA8(strategy) => super::luma_a8::ops::execute(strategy),
            ReduceStrategy::ImageLaPremultiplied8(strategy) => {
                super::la_premultiplied8::ops::execute(strategy)
            }
            ReduceStrategy::ImagePalette8(strategy) => super::palette8::ops::execute(strategy),
            ReduceStrategy::ImagePaletteAlpha8(strategy) => {
                super::palette_alpha8::ops::execute(strategy)
            }
            ReduceStrategy::ImageRgb8(strategy) => super::rgb8::ops::execute(strategy),
            ReduceStrategy::ImageRgba8(strategy) => super::rgba8::ops::execute(strategy),
            ReduceStrategy::ImageRgbPremultiplied8(strategy) => {
                super::rgb_premultiplied8::ops::execute(strategy)
            }
            ReduceStrategy::ImageRgbx8(strategy) => super::rgbx8::ops::execute(strategy),
            ReduceStrategy::ImageCmyk8(strategy) => super::cmyk8::ops::execute(strategy),
            ReduceStrategy::ImageYCbCr8(strategy) => super::ycbcr8::ops::execute(strategy),
            ReduceStrategy::ImageLab8(strategy) => super::lab8::ops::execute(strategy),
            ReduceStrategy::ImageHsv8(strategy) => super::hsv8::ops::execute(strategy),
            ReduceStrategy::ImageI32(strategy) => super::i32::ops::execute(strategy),
            ReduceStrategy::ImageF32(strategy) => super::f32::ops::execute(strategy),
            ReduceStrategy::ImageI16(strategy) => super::i16::ops::execute(strategy),
            ReduceStrategy::ImageI16L(strategy) => super::i16l::ops::execute(strategy),
            ReduceStrategy::ImageI16B(strategy) => super::i16b::ops::execute(strategy),
            ReduceStrategy::ImageI16N(strategy) => super::i16n::ops::execute(strategy),
            ReduceStrategy::ImageLumaA16(strategy) => super::luma_a16::ops::execute(strategy),
            ReduceStrategy::ImageRgb16(strategy) => super::rgb16::ops::execute(strategy),
            ReduceStrategy::ImageRgba16(strategy) => super::rgba16::ops::execute(strategy),
            ReduceStrategy::ImageRgb32F(strategy) => super::rgb32f::ops::execute(strategy),
            ReduceStrategy::ImageRgba32F(strategy) => super::rgba32f::ops::execute(strategy),
        }?;
        super::helpers::hidden_lanes::finish(image, &mut output, factors)?;
        Ok(output)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
