//! Transpose-family operation support and prepared mode strategy composition.
//!
//! Archived portable wide-lane transpose paths now have explicit per-mode
//! SSE2, NEON, and Wasm SIMD128 strategies. RGBA keeps its AVX2 axis-swap
//! strategy ahead of the native-byte route, and packed-word Rotate270 lives in
//! its Default mode owner.
//!
//! TODO(pillow-rs#3ptb): move parallel transpose collection kernels to the later
//! `pool_parallel` stage. Retire the duplicate SIMD byte Flip route only after
//! all CPU callers use these prepared strategies.

// NO_PILLOW_SUPPORT: Internal high-depth raster identities retain their exact
// sample types and mode tags through these native transpose strategies.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport, OperationId, OperationInputs, OperationSupport, RequestFacts, StrategyInputs,
    StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, TransposeMethod};
use crate::raster::{DynamicImage, Mode};

macro_rules! declare_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedFlip<'a> {
            $($mode(super::$module::ops::FlipStrategy<'a>),)+
        }
        pub(crate) enum PreparedMirror<'a> {
            $($mode(super::$module::ops::MirrorStrategy<'a>),)+
        }
        pub(crate) enum PreparedTranspose<'a> {
            $($mode(super::$module::ops::TransposeStrategy<'a>),)+
        }

        fn classify_flip(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::FLIP_SUPPORT,)+ }
        }
        fn classify_mirror(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::MIRROR_SUPPORT,)+ }
        }
        fn classify_transpose(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::TRANSPOSE_SUPPORT,)+ }
        }

        fn prepare_flip_mode<'a>(
            image: &'a DynamicImage,
            ctx: &ExecutionContext,
        ) -> Result<PreparedFlip<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(image) => super::$module::ops::prepare_flip(image, ctx)
                    .map(PreparedFlip::$mode),)+
            }
        }

        fn prepare_mirror_mode<'a>(
            image: &'a DynamicImage,
            ctx: &ExecutionContext,
        ) -> Result<PreparedMirror<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(image) => super::$module::ops::prepare_mirror(image, ctx)
                    .map(PreparedMirror::$mode),)+
            }
        }

        fn prepare_transpose_mode<'a>(
            image: &'a DynamicImage,
            method: &TransposeMethod,
            facts: StrategyInputs,
            ctx: &ExecutionContext,
        ) -> Result<PreparedTranspose<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(image) => super::$module::ops::prepare_transpose(image, method, facts, ctx)
                    .map(PreparedTranspose::$mode),)+
            }
        }

        impl PreparedFlip<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                let image = match self { $(Self::$mode(strategy) => strategy.execute(),)+ };
                Ok(image)
            }
        }

        impl PreparedMirror<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                let image = match self { $(Self::$mode(strategy) => strategy.execute(),)+ };
                Ok(image)
            }
        }

        impl PreparedTranspose<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => strategy.execute(),)+ }
            }
        }
    };
}

declare_modes! {
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

/// Complete mode and architecture contract for vertical flip.
pub(crate) static FLIP_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Flip, &[], classify_flip);

/// Complete mode and architecture contract for horizontal mirror.
pub(crate) static MIRROR_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Mirror, &[], classify_mirror);

/// Complete mode and architecture contract for Pillow transpose methods.
pub(crate) static TRANSPOSE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Transpose, &[], classify_transpose);

pub(crate) fn prepare_flip<'a>(
    image: &'a DynamicImage,
    ctx: &ExecutionContext,
) -> Result<PreparedFlip<'a>, PilError> {
    prepare_flip_mode(image, ctx)
}

pub(crate) fn prepare_mirror<'a>(
    image: &'a DynamicImage,
    ctx: &ExecutionContext,
) -> Result<PreparedMirror<'a>, PilError> {
    prepare_mirror_mode(image, ctx)
}

pub(crate) fn execute_flip(
    image: &DynamicImage,
    ctx: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    prepare_flip(image, ctx)?.execute()
}

pub(crate) fn execute_mirror(
    image: &DynamicImage,
    ctx: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    prepare_mirror(image, ctx)?.execute()
}

pub(crate) fn prepare_transpose<'a>(
    image: &'a DynamicImage,
    method: &TransposeMethod,
    ctx: &ExecutionContext,
) -> Result<PreparedTranspose<'a>, PilError> {
    let mut facts = OperationInputs { image, palette: Default::default() }.facts();
    facts.request = RequestFacts::Transpose { method: *method };
    prepare_transpose_mode(image, method, facts, ctx)
}

pub(crate) fn assess_transpose(
    image: &DynamicImage,
    method: &TransposeMethod,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let mut facts = OperationInputs { image, palette: Default::default() }.facts();
    facts.request = RequestFacts::Transpose { method: *method };
    TRANSPOSE_SUPPORT.for_mode(image.mode()).assess(ctx, facts)
}

pub(crate) fn execute_transpose(
    image: &DynamicImage,
    method: &TransposeMethod,
    ctx: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    prepare_transpose(image, method, ctx)?.execute()
}

pub(crate) fn prepare_flip_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::Flip) {
        return Err(PilError::ValueError("expected Flip op".into()));
    }
    prepare_flip(input.image, ctx).map(crate::compute::registry::PreparedOperation::from_flip)
}

pub(crate) fn prepare_mirror_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::Mirror) {
        return Err(PilError::ValueError("expected Mirror op".into()));
    }
    prepare_mirror(input.image, ctx)
        .map(crate::compute::registry::PreparedOperation::from_mirror)
}

pub(crate) fn prepare_transpose_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Transpose { method } = op else {
        return Err(PilError::ValueError("expected Transpose op".into()));
    };
    prepare_transpose(input.image, method, ctx)
        .map(crate::compute::registry::PreparedOperation::from_transpose)
}
