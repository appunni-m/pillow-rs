//! Typed band extraction and native writes with exhaustive source-mode strategies.
//!
//! Pipeline callbacks for PutPixel, PutData, PutAlpha, and PutAlphaData are
//! registered and consumed by the serial CPU batch executor. Consecutive
//! PutPixel runs also compose with the prepared RGB Thumbnail fusion.
//! TODO(pillow-rs#3ptb): finish auditing wrapper metadata paths for
//! Backend/Architecture context bypasses and reconcile mode-changing callers.
//! Immediate PutData callbacks use their own exact-mode support declaration and
//! a sequence-scoped CPU Default execution context.
//! GetData's direct output-buffer route uses the prepared ExtractBand strategy.
//! Source review confirmed PutData's cross-carrier ConvertBuffer routes are
//! semantic: they construct the requested 1/2/3/4-channel payload carrier
//! before applying the byte prefix, matching the archived operation. Same-
//! carrier paths write the exact source carrier directly and preserve its mode.
//! PutAlpha, PutAlphaData, and ExtractBand conversion routes were reviewed
//! against the archived contracts. Mask-to-L normalization remains semantic;
//! exact one-channel masks borrow their bytes in Default paths. Same-carrier
//! alpha outputs keep their single required result copy, and ExtractBand reads
//! native channels directly. This source review remains unvalidated.
// NO_PILLOW_SUPPORT: Internal high-depth raster identities have declared
// core-only band and mutation routes; their narrowing and output rules remain
// explicit in each mode contract.
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

            pub(crate) fn execute_into(
                self,
                output: &mut [u8],
            ) -> Result<Option<(u64, u64, &'static str)>, PilError> {
                match self {
                    Self::ImageLumaA8(strategy) => super::luma_a8::ops::execute_into(strategy, output).map(Some),
                    Self::ImagePaletteAlpha8(strategy) =>
                        super::palette_alpha8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageRgb8(strategy) => super::rgb8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageRgba8(strategy) => super::rgba8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageRgbPremultiplied8(strategy) =>
                        super::rgb_premultiplied8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageRgbx8(strategy) => super::rgbx8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageCmyk8(strategy) => super::cmyk8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageHsv8(strategy) => super::hsv8::ops::execute_into(strategy, output).map(Some),
                    Self::ImageYCbCr8(strategy) => super::ycbcr8::ops::execute_into(strategy, output).map(Some),
                    _ => Ok(None),
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

const PUT_DATA_L16_BYTES_INPUTS: &[crate::compute::registry::support::InputContract] =
    &[crate::compute::registry::support::InputContract::CallerValidatedRawI16ByteRows];

fn classify_put_data_l16_bytes_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            ModeSupport::serial(OperationId::PutDataL16Bytes, mode, PUT_DATA_L16_BYTES_INPUTS)
        }
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
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "putdata I;16 storage mismatch",
            kind: crate::compute::registry::support::RejectionKind::InternalError,
        },
    }
}

pub(crate) static PUT_DATA_L16_BYTES_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutDataL16Bytes,
    &[],
    classify_put_data_l16_bytes_mode,
);

pub(crate) fn select_put_data_l16_bytes(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    PUT_DATA_L16_BYTES_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

pub(crate) fn prepare_put_data_l16_bytes(
    data: &[u8],
    dimensions: crate::checked_dims::CheckedDims,
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    super::helpers::put_data_l16_bytes::prepare_raw_bytes(data, dimensions, scale, offset)
}

pub(crate) fn execute_put_data_l16_bytes(
    image: &mut DynamicImage,
    raw: &[u8],
    selection: StrategySelection,
) -> Result<(), PilError> {
    let mode = image.mode();
    if selection.implementation.operation != OperationId::PutDataL16Bytes
        || selection.implementation.mode != mode
    {
        return Err(PilError::InternalError(
            "putdata I;16 storage mismatch".into(),
        ));
    }
    if selection.implementation.architecture != crate::compute::Architecture::Default {
        return Err(PilError::InternalError(
            "unbound putdata I;16 bytes architecture".into(),
        ));
    }
    match image {
        DynamicImage::ImageI16(image) => {
            super::i16::default::put_data_impl::execute_l16_bytes(image, raw)
        }
        DynamicImage::ImageI16L(image) => {
            super::i16l::default::put_data_impl::execute_l16_bytes(image, raw)
        }
        DynamicImage::ImageI16B(image) => {
            super::i16b::default::put_data_impl::execute_l16_bytes(image, raw)
        }
        DynamicImage::ImageI16N(image) => {
            super::i16n::default::put_data_impl::execute_l16_bytes(image, raw)
        }
        _ => Err(PilError::InternalError(
            "putdata I;16 storage mismatch".into(),
        )),
    }
}

const PUT_DATA_VALUE_AT_INPUTS: &[crate::compute::registry::support::InputContract] =
    &[crate::compute::registry::support::InputContract::CallerValidatedPixelIndex];

fn classify_data_value_at_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::PutDataValueAt,
            mode,
            crate::compute::registry::support::OutputModeRule::Preserve,
            PUT_DATA_VALUE_AT_INPUTS,
            &[],
        ),
        Mode::ImageLaPremultiplied8 => ModeSupport::Rejected {
            mode,
            reason: "unsupported putdata mode: La",
            kind: crate::compute::registry::support::RejectionKind::ValueError,
        },
        Mode::ImageLab8 => ModeSupport::Rejected {
            mode,
            reason: "unsupported putdata mode: LAB",
            kind: crate::compute::registry::support::RejectionKind::ValueError,
        },
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "unsupported putdata mode: unknown",
            kind: crate::compute::registry::support::RejectionKind::ValueError,
        },
    }
}

pub(crate) static PUT_DATA_VALUE_AT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutDataValueAt,
    &[],
    classify_data_value_at_mode,
);

const PUT_PIXEL_VALUE_AT_INPUTS: &[crate::compute::registry::support::InputContract] =
    &[crate::compute::registry::support::InputContract::CallerValidatedPixelIndex];

fn classify_pixel_value_at_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::serial(
            OperationId::PutPixelValueAt,
            mode,
            PUT_PIXEL_VALUE_AT_INPUTS,
        ),
        _ => ModeSupport::Rejected {
            mode,
            reason: "scalar putpixel value-at requires I, F, or I;16 mode",
            kind: crate::compute::registry::support::RejectionKind::NotImplementedError,
        },
    }
}

pub(crate) static PUT_PIXEL_VALUE_AT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutPixelValueAt,
    &[],
    classify_pixel_value_at_mode,
);

pub(crate) fn select_put_pixel_value_at(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    PUT_PIXEL_VALUE_AT_SUPPORT.for_mode(mode).select_serial(context)
}

pub(crate) fn execute_put_pixel_value_at(
    image: &mut DynamicImage,
    pixel_index: usize,
    input: PutPixelValueAtInput,
    selection: StrategySelection,
) -> Result<(), PilError> {
    if selection.implementation.operation != OperationId::PutPixelValueAt {
        return Err(PilError::InternalError(
            "putpixel value-at strategy does not match its raster mode".into(),
        ));
    }
    let mode = selection.implementation.mode;
    if image.mode() != mode {
        let is_l16 = matches!(
            mode,
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
        );
        let error = if is_l16 {
            "putpixel I;16 storage mismatch"
        } else {
            "putdata RGBA storage mismatch"
        };
        return Err(PilError::InternalError(error.into()));
    }
    if selection.implementation.architecture != crate::compute::Architecture::Default {
        return Err(PilError::InternalError(
            "unbound putpixel value-at architecture".into(),
        ));
    }
    match (image, input) {
        (DynamicImage::ImageI32(image), PutPixelValueAtInput::NumericBytes(bytes)) => {
            super::i32::default::put_pixel_impl::execute_value_at(image, pixel_index, bytes)
        }
        (DynamicImage::ImageF32(image), PutPixelValueAtInput::NumericBytes(bytes)) => {
            super::f32::default::put_pixel_impl::execute_value_at(image, pixel_index, bytes)
        }
        (DynamicImage::ImageI16(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (DynamicImage::ImageI16L(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16l::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (DynamicImage::ImageI16B(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16b::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (DynamicImage::ImageI16N(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16n::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        _ => Err(PilError::InternalError(
            "putpixel value-at input does not match its raster mode".into(),
        )),
    }
}

pub(crate) enum PutPixelValueAtInput {
    NumericBytes([u8; 4]),
    Luma16(u16),
}

pub(crate) fn select_put_data_value_at(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    PUT_DATA_VALUE_AT_SUPPORT.for_mode(mode).select_serial(context)
}

pub(crate) fn execute_put_data_value_at(
    image: &mut DynamicImage,
    pixel_index: usize,
    input: super::helpers::put_data_value_at::ValueAtInput,
    selection: StrategySelection,
) -> Result<(), PilError> {
    let mode = image.mode();
    if selection.implementation.operation != OperationId::PutDataValueAt
        || selection.implementation.mode != mode
    {
        return Err(PilError::InternalError(
            "putdata value-at strategy does not match its raster mode".into(),
        ));
    }
    if selection.implementation.architecture != crate::compute::Architecture::Default {
        return Err(PilError::InternalError(
            "unbound putdata value-at architecture".into(),
        ));
    }
    super::helpers::put_data_value_at::execute(image, mode, pixel_index, input)
}

pub(crate) static PUT_ALPHA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutAlpha, &[], classify_alpha_mode,
);
pub(crate) static PUT_ALPHA_DATA_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutAlphaData, &[], classify_alpha_data_mode,
);

pub(crate) fn prepare_alpha_pipeline<'a>(
    input: OperationInputs<'a>, op: &'a PipelineOp, ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutAlpha { alpha, mode } = op else {
        return Err(PilError::ValueError("expected PutAlpha op".into()));
    };
    if input.image.mode() != *mode {
        return Err(PilError::InternalError(
            "PutAlpha source mode does not match its DynamicImage".into(),
        ));
    }
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
    let PipelineOp::PutAlphaData { mask, mode } = op else {
        return Err(PilError::ValueError("expected PutAlphaData op".into()));
    };
    if input.image.mode() != *mode {
        return Err(PilError::InternalError(
            "PutAlphaData source mode does not match its DynamicImage".into(),
        ));
    }
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
    let PipelineOp::PutData { data, payload_mode } = op else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    prepare_put_data(input.image, data, *payload_mode, ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_data)
}
