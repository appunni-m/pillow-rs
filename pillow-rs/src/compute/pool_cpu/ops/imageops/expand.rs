//! ImageOps.expand's exhaustive exact-mode contract and prepared strategies.
//!
//! The operation adds native fill samples around each source raster and keeps
//! the exact input identity. Internal high-depth strategies are marked beside
//! their native sample implementations.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ExpandFillRule, InputContract, ModeSupport, OperationId, OperationInputs,
    OperationSupport, StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::error::PilError;
use crate::pipeline::{NativeColorInput, PipelineOp};
use crate::raster::{DynamicImage, Mode};

macro_rules! declare_expand_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedExpand<'a> {
            $($mode(super::$module::expand::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::expand::SUPPORT,)+
            }
        }

        fn prepare_image<'a>(
            image: &'a DynamicImage,
            border: crate::pipeline::ExpandMargins,
            fill: (u8, u8, u8, u8),
            ctx: &ExecutionContext,
        ) -> Result<PreparedExpand<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::expand::prepare(
                    input, border, fill, ctx,
                ).map(PreparedExpand::$mode),)+
            }
        }

        impl PreparedExpand<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self {
                    $(Self::$mode(strategy) => strategy.selection(),)+
                }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::expand::execute(strategy),)+
                }
            }
        }
    };
}

declare_expand_modes! {
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

/// Complete exact-mode support for native ImageOps.expand.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Expand, &[], classify_mode);

pub(crate) fn assess(
    mode: Mode,
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    classify_mode(mode).assess(ctx, input)
}

pub(crate) fn prepare(
    image: &DynamicImage,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    ctx: &ExecutionContext,
) -> Result<PreparedExpand<'_>, PilError> {
    prepare_image(image, border, fill, ctx)
}

// TODO(pillow-rs#3ptb): Freeze I;16 low-16-bit fill wrap, CoreOnly typed
// component fills, and Default/lower-architecture fallback receipts in CPU parity.
pub(crate) fn prepare_with_native_fill(
    image: &DynamicImage,
    border: crate::pipeline::ExpandMargins,
    fill: (u8, u8, u8, u8),
    native_fill: NativeColorInput,
    ctx: &ExecutionContext,
) -> Result<PreparedExpand<'_>, PilError> {
    let support = SUPPORT.for_mode(image.mode());
    let declares_native_fill = support.inputs.iter().any(|contract| {
        matches!(
            contract,
            InputContract::ExpandFill(
                ExpandFillRule::NativeU16 | ExpandFillRule::NativeF32
            )
        )
    });
    if !declares_native_fill {
        return Err(PilError::InternalError(
            "typed Expand fill reached a mode without a native fill contract".into(),
        ));
    }
    match image {
        DynamicImage::ImageI16(input) => super::i16::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageI16),
        DynamicImage::ImageI16L(input) => super::i16l::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageI16L),
        DynamicImage::ImageI16B(input) => super::i16b::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageI16B),
        DynamicImage::ImageI16N(input) => super::i16n::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageI16N),
        DynamicImage::ImageLumaA16(input) => super::luma_a16::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageLumaA16),
        DynamicImage::ImageRgb16(input) => super::rgb16::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageRgb16),
        DynamicImage::ImageRgba16(input) => super::rgba16::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageRgba16),
        DynamicImage::ImageRgb32F(input) => super::rgb32f::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageRgb32F),
        DynamicImage::ImageRgba32F(input) => super::rgba32f::expand::prepare_with_native_fill(
            input, border, fill, Some(native_fill), ctx,
        )
        .map(PreparedExpand::ImageRgba32F),
        _ => Err(PilError::InternalError(
            "typed Expand fill reached a non-native-width mode".into(),
        )),
    }
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    match op {
        PipelineOp::Expand { border, fill } => prepare(input.image, *border, *fill, ctx),
        PipelineOp::ExpandTyped { border, fill, input: native_fill } => prepare_with_native_fill(
            input.image,
            *border,
            *fill,
            *native_fill,
            ctx,
        ),
        _ => return Err(PilError::ValueError("expected Expand op".into())),
    }
    .map(crate::compute::registry::PreparedOperation::from_expand)
}
