//! Typed band extraction and native writes with exhaustive source-mode strategies.
//!
//! TODO(pillow-rs#3ptb): connect these prepared plans to the serial pool/batch
//! executor and thumbnail fusion, migrate the remaining public metadata
//! boundaries, and retire the separate SIMD eager route.
//! TODO(pillow-rs#3ptb): review every cross-mode conversion in PutData,
//! PutAlpha, PutAlphaData, and ExtractBand against the archived contract. Keep
//! conversions only when they are part of that operation's Pillow semantics;
//! replace conversions used only for kernel reuse with native-mode code.
// NO_PILLOW_SUPPORT: Internal high-depth raster identities have declared
// core-only band and mutation routes. The TODO below still audits their exact
// sample semantics and removes conversions used only for implementation reuse.
//!
//! This internal operation retains its original defensive index behavior.
//! Image::getchannel validates public selectors, rejects I/F, and preserves
//! P-mode identity before queuing it; those wrapper contracts remain separate.

use crate::compute::{ExecutionContext, registry::support::{
    ModeSupport, OperationId, OperationInputs, OperationSupport, StrategyInputs,
    StrategySelection, SupportAssessment,
}};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::helpers::put_pixel::PixelWrite;

macro_rules! declare_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedExtractBand<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }
        pub(crate) enum PreparedPutData<'a> {
            $($mode(super::$module::ops::DataStrategy<'a>),)+
        }
        pub(crate) enum PreparedPutAlpha<'a> {
            $($mode(super::$module::ops::AlphaStrategy<'a>),)+
        }
        pub(crate) enum PreparedPutAlphaData<'a> {
            $($mode(super::$module::ops::AlphaDataStrategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::SUPPORT,)+ }
        }
        fn classify_data_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_DATA_SUPPORT,)+ }
        }
        fn classify_alpha_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_ALPHA_SUPPORT,)+ }
        }
        fn classify_alpha_data_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_ALPHA_DATA_SUPPORT,)+ }
        }

        pub(crate) fn assess(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode { $(Mode::$mode => super::$module::ops::assess(input, ctx),)+ }
        }

        pub(crate) fn prepare<'a>(image: &'a DynamicImage, index: u8, ctx: &ExecutionContext)
            -> Result<PreparedExtractBand<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, index, ctx)
                    .map(PreparedExtractBand::$mode),)+
            }
        }

        pub(crate) fn assess_put_data(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode { $(Mode::$mode => super::$module::ops::assess_data(input, ctx),)+ }
        }

        pub(crate) fn prepare_put_data<'a>(
            image: &'a DynamicImage, data: &'a [u8], payload: Mode, ctx: &ExecutionContext,
        ) -> Result<PreparedPutData<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) =>
                    super::$module::ops::prepare_data(input, data, payload, ctx)
                        .map(PreparedPutData::$mode),)+
            }
        }

        pub(crate) fn assess_put_alpha(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode { $(Mode::$mode => super::$module::ops::assess_alpha(input, ctx),)+ }
        }

        pub(crate) fn prepare_put_alpha<'a>(
            image: &'a DynamicImage, alpha: u8, ctx: &ExecutionContext,
        ) -> Result<PreparedPutAlpha<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare_alpha(input, alpha, ctx)
                    .map(PreparedPutAlpha::$mode),)+
            }
        }

        pub(crate) fn assess_put_alpha_data(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode { $(Mode::$mode => super::$module::ops::assess_alpha_data(input, ctx),)+ }
        }

        pub(crate) fn prepare_put_alpha_data<'a>(
            image: &'a DynamicImage, mask: &DynamicImage, ctx: &ExecutionContext,
        ) -> Result<PreparedPutAlphaData<'a>, PilError> {
            let mask = super::helpers::alpha_mask::MaskStrategy::new(mask);
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare_alpha_data(input, mask, ctx)
                    .map(PreparedPutAlphaData::$mode),)+
            }
        }

        impl PreparedExtractBand<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+
                }
            }
        }

        impl PreparedPutData<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => strategy.execute(),)+ }
            }
        }

        impl PreparedPutAlpha<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => strategy.execute(),)+ }
            }
        }

        impl PreparedPutAlphaData<'_> {
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

pub(crate) static EXTRACT_BAND_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ExtractBand, &[], classify_mode,
);
pub(crate) static PUT_DATA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutData, &[], classify_data_mode,
);
pub(crate) static PUT_ALPHA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutAlpha, &[], classify_alpha_mode,
);
pub(crate) static PUT_ALPHA_DATA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutAlphaData, &[], classify_alpha_data_mode,
);

pub(crate) fn prepare_alpha_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutAlpha { alpha, .. } = op else {
        return Err(PilError::ValueError("expected PutAlpha op".into()));
    };
    prepare_put_alpha(input.image, *alpha, ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_alpha)
}

pub(crate) fn assess_alpha_data(
    image: &DynamicImage, mask: &DynamicImage, ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    let mut facts = OperationInputs { image, palette: Default::default() }.facts();
    facts.request = crate::compute::registry::support::RequestFacts::PutAlphaData {
        mask: crate::compute::registry::support::SecondaryInputFacts {
            mode: mask.mode(), width: mask.width(), height: mask.height(),
            byte_len: Some(mask.as_bytes().len()),
        },
    };
    assess_put_alpha_data(image.mode(), facts, ctx)
}

pub(crate) fn prepare_alpha_data_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutAlphaData { mask, .. } = op else {
        return Err(PilError::ValueError("expected PutAlphaData op".into()));
    };
    prepare_put_alpha_data(input.image, mask.as_ref(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_alpha_data)
}

pub(crate) fn execute_extract_band(image: &DynamicImage, index: u8, ctx: &ExecutionContext)
    -> Result<DynamicImage, PilError> {
    prepare(image, index, ctx)?.execute()
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::ExtractBand { index } = op else {
        return Err(PilError::ValueError("expected ExtractBand op".into()));
    };
    prepare(input.image, *index, ctx)
        .map(crate::compute::registry::PreparedOperation::from_extract_band)
}

const WRITE_REJECTION_REASON: &str = "putpixel not supported for this image type";
const WRITE_REJECTION_KIND: crate::compute::registry::support::RejectionKind =
    crate::compute::registry::support::RejectionKind::NotImplementedError;

fn rejected_write(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: WRITE_REJECTION_REASON,
        kind: WRITE_REJECTION_KIND,
    }
}

macro_rules! declare_write_modes {
    (supported { $($mode:ident => $module:ident),+ $(,)? }
        rejected { $($rejected:ident),+ $(,)? }
    ) => {
        pub(crate) enum PreparedPutPixel<'a> {
            $($mode(super::$module::ops::PixelStrategy<'a>),)+
        }
        pub(crate) enum PreparedPutPixelBatch<'a> {
            $($mode(super::$module::ops::BatchStrategy<'a>),)+
        }

        fn classify_pixel_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::PUT_PIXEL_SUPPORT,)+
                $(Mode::$rejected => rejected_write(mode),)+
            }
        }

        fn classify_pixel_batch_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::PUT_PIXEL_BATCH_SUPPORT,)+
                $(Mode::$rejected => rejected_write(mode),)+
            }
        }

        pub(crate) fn assess_put_pixel(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::assess_pixel(input, ctx),)+
                $(Mode::$rejected => rejected_write(mode).assess(ctx, input),)+
            }
        }

        pub(crate) fn assess_put_pixel_batch(mode: Mode, input: StrategyInputs, ctx: &ExecutionContext)
            -> Result<SupportAssessment, PilError> {
            match mode {
                $(Mode::$mode => super::$module::ops::assess_pixel_batch(input, ctx),)+
                $(Mode::$rejected => rejected_write(mode).assess(ctx, input),)+
            }
        }

        pub(crate) fn prepare_put_pixel<'a>(
            image: &'a DynamicImage, write: PixelWrite, ctx: &ExecutionContext,
        ) -> Result<PreparedPutPixel<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) =>
                    super::$module::ops::prepare_pixel(input, write, ctx)
                        .map(PreparedPutPixel::$mode),)+
                $(DynamicImage::$rejected(_) =>
                    Err(WRITE_REJECTION_KIND.into_error(WRITE_REJECTION_REASON)),)+
            }
        }

        pub(crate) fn prepare_put_pixel_batch<'a>(
            image: &'a DynamicImage, ops: &'a [PipelineOp], ctx: &ExecutionContext,
        ) -> Result<PreparedPutPixelBatch<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) =>
                    super::$module::ops::prepare_pixel_batch(input, ops, ctx)
                        .map(PreparedPutPixelBatch::$mode),)+
                $(DynamicImage::$rejected(_) =>
                    Err(WRITE_REJECTION_KIND.into_error(WRITE_REJECTION_REASON)),)+
            }
        }

        impl PreparedPutPixel<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => strategy.execute(),)+ }
            }
        }

        impl PreparedPutPixelBatch<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => strategy.execute(),)+ }
            }
        }
    };
}

declare_write_modes! {
    supported {
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
    }
    rejected {
        ImageI16, ImageI16L, ImageI16B, ImageI16N,
        ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, ImageRgba32F,
    }
}

pub(crate) static PUT_PIXEL_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutPixel, &[], classify_pixel_mode,
);
pub(crate) static PUT_PIXEL_BATCH_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutPixelBatch, &[], classify_pixel_batch_mode,
);

pub(crate) fn prepare_pixel_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutPixel { x, y, color, .. } = op else {
        return Err(PilError::ValueError("expected PutPixel op".into()));
    };
    prepare_put_pixel(input.image, PixelWrite { x: *x, y: *y, color: *color }, ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_pixel)
}

pub(crate) fn prepare_data_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutData { data, mode } = op else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    prepare_put_data(input.image, data, mode.raster_mode(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_data)
}
