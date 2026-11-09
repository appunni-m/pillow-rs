//! Typed band extraction and native writes with exhaustive source-mode strategies.
//!
//! Pipeline callbacks for PutPixel, PutData, PutAlpha, and PutAlphaData are
//! registered and consumed by the serial CPU batch executor. Consecutive
//! PutPixel runs also compose with the prepared RGB Thumbnail fusion.
//! TODO(pillow-rs#3ptb): verify PutAlpha/PutAlphaData mode changes and
//! validation/error ordering, including callback-observed promotion and
//! destination changes, noncanonical One masks, La scalar/mask output, declared
//! rejections, CoreOnly high-depth outputs, and RGBa alpha values 0, 1, 254,
//! and 255 with constant and mask inputs in CPU parity. Verify ImagingCore
//! PutBand/FillBand outputs, error ordering, compact hidden-lane retention, and
//! serial Default fallback receipts across every declared mode.
//! Wrapper mode validation and transitions now use these prepared declarations;
//! source review found no separate Backend/Architecture selection in the
//! wrappers. Cover callback PutData/PutPixel selection, including La/LAB
//! packed/component lane retention, exact-mode band validation, GetChannel
//! selection receipts, and deferred terminal receipts.
//! Immediate PutData callbacks use their exact-mode support declaration and a
//! sequence-scoped routed CPU context; a missing architecture specialization
//! selects the native Default implementation.
//! GetData's direct output-buffer route uses the prepared ExtractBand strategy.
//! Source review confirmed PutData's cross-carrier ConvertBuffer routes are
//! semantic: they construct the requested 1/2/3/4-channel payload carrier
//! before applying the byte prefix, matching the archived operation. Same-
//! carrier paths write the exact source carrier directly and preserve its mode.
//! PutAlpha, PutAlphaData, and ExtractBand conversion routes were compared with
//! the archive. RGBa and La alpha replacement convert to straight alpha before
//! replacing alpha because archived generic carrier routes retained
//! premultiplied samples. One-mask-to-L normalization is fused into exact-mode
//! Default and vector PutAlphaData loops; L mask samples stay borrowed directly.
//! Same-carrier alpha outputs keep their single required result copy,
//! and ExtractBand reads native channels directly. This source review remains
//! unvalidated.
// NO_PILLOW_SUPPORT: Internal high-depth raster identities have declared
// core-only band, GetChannel, and mutation routes; ExtractBand preserves u16
// and f32 samples in their native scalar output modes.
//!
//! This internal operation retains its original defensive index behavior.
//! Image::getchannel validates public selectors, rejects I/F, and preserves
//! P-mode identity before queuing it; GetChannel declares those wrapper routes
//! at its operation-level support entry.

use crate::compute::{
    ExecutionContext,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OperationSupport, RejectionKind,
        StrategySelection,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::imaging_core_set_mode::{
    IMAGING_CORE_SET_MODE_SUPPORT, execute as execute_imaging_core_set_mode,
    select as select_imaging_core_set_mode,
};

pub(crate) use super::helpers::put_pixel::PixelWrite;

// Keep support classification exhaustive while limiting alpha execution
// enums to supported modes; accepted and rejected lists make the image match
// exhaustive so a newly added Mode cannot silently fall through.
macro_rules! declare_modes {
    (
        all { $($mode:ident => $module:ident),+ $(,)? }
        alpha { $($alpha_mode:ident => $alpha_module:ident),+ $(,)? }
        rejected_alpha { $($rejected_mode:ident),+ $(,)? }
    ) => {
        pub(crate) enum PreparedExtractBand<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }
        pub(crate) enum PreparedPutData<'a> {
            $($mode(
                super::$module::ops::DataStrategy<'a>,
                super::helpers::put_data::HiddenLaneUpdate<'a>,
            ),)+
        }
        pub(crate) enum PreparedPutAlpha<'a> {
            $($alpha_mode(super::$alpha_module::ops::AlphaStrategy<'a>),)+
        }
        pub(crate) enum PreparedPutAlphaData<'a> {
            $($alpha_mode(super::$alpha_module::ops::AlphaDataStrategy<'a>),)+
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
        fn classify_data_value_at_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_DATA_VALUE_AT_SUPPORT,)+ }
        }
        fn classify_pixel_value_at_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_PIXEL_VALUE_AT_SUPPORT,)+ }
        }
        fn classify_put_data_l16_bytes_mode(mode: Mode) -> ModeSupport {
            match mode { $(Mode::$mode => super::$module::ops::PUT_DATA_L16_BYTES_SUPPORT,)+ }
        }


        pub(crate) fn prepare<'a>(image: &'a DynamicImage, index: u8, ctx: &ExecutionContext)
            -> Result<PreparedExtractBand<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(input) => super::$module::ops::prepare(input, index, ctx)
                    .map(PreparedExtractBand::$mode),)+
            }
        }


        pub(crate) fn prepare_put_data<'a>(
            image: &'a DynamicImage,
            data: &'a [u8],
            payload: Mode,
            hidden_lanes: Option<bytes::Bytes>,
            ctx: &ExecutionContext,
        ) -> Result<PreparedPutData<'a>, PilError> {
            let lane_update = super::helpers::put_data::HiddenLaneUpdate::new(
                image,
                payload,
                data.len(),
                hidden_lanes,
            );
            match image {
                $(DynamicImage::$mode(input) =>
                    super::$module::ops::prepare_data(input, data, payload, ctx)
                        .map(|strategy| PreparedPutData::$mode(strategy, lane_update)),)+
            }
        }


        pub(crate) fn prepare_put_alpha<'a>(
            image: &'a DynamicImage, alpha: u8, ctx: &ExecutionContext,
        ) -> Result<PreparedPutAlpha<'a>, PilError> {
            match image {
                $(DynamicImage::$alpha_mode(input) => super::$alpha_module::ops::prepare_alpha(input, alpha, ctx)
                    .map(PreparedPutAlpha::$alpha_mode),)+
                $(DynamicImage::$rejected_mode(_) =>
                    Err(rejected_alpha_mode_error(Mode::$rejected_mode)),)+
            }
        }


        pub(crate) fn prepare_put_alpha_data<'a>(
            image: &'a DynamicImage, mask: &'a DynamicImage, ctx: &ExecutionContext,
        ) -> Result<PreparedPutAlphaData<'a>, PilError> {
            classify_alpha_data_mode(image.mode()).validate_mode()?;
            let mask = super::helpers::alpha_mask::MaskStrategy::new(mask)?;
            match image {
                $(DynamicImage::$alpha_mode(input) => super::$alpha_module::ops::prepare_alpha_data(input, mask, ctx)
                    .map(PreparedPutAlphaData::$alpha_mode),)+
                $(DynamicImage::$rejected_mode(_) =>
                    Err(rejected_alpha_data_mode_error(Mode::$rejected_mode)),)+
            }
        }

        impl PreparedExtractBand<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+ }
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
                match self { $(Self::$mode(strategy, _) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy, update) => {
                        let mut output = strategy.execute()?;
                        super::helpers::put_data::apply_hidden_lane_update(&mut output, update)?;
                        Ok(output)
                    },)+
                }
            }
        }

        impl PreparedPutAlpha<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$alpha_mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$alpha_mode(strategy) => strategy.execute(),)+ }
            }
        }

        impl PreparedPutAlphaData<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$alpha_mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self { $(Self::$alpha_mode(strategy) => strategy.execute(),)+ }
            }
        }
    };
}

declare_modes! {
    all {
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
    alpha {
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
    // These six rejections are declared by put_alpha_strategy::support.
    rejected_alpha {
        One,
        ImageYCbCr8,
        ImageLab8,
        ImageHsv8,
        ImageI32,
        ImageF32,
    }
}

fn rejected_alpha_mode_error(mode: Mode) -> PilError {
    classify_alpha_mode(mode)
        .validate_mode()
        .err()
        .unwrap_or_else(|| PilError::InternalError("PutAlpha support has no mode strategy".into()))
}

fn rejected_alpha_data_mode_error(mode: Mode) -> PilError {
    classify_alpha_data_mode(mode)
        .validate_mode()
        .err()
        .unwrap_or_else(|| {
            PilError::InternalError("PutAlphaData support has no mode strategy".into())
        })
}

pub(crate) static EXTRACT_BAND_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::ExtractBand, &[], classify_mode);

pub(crate) static PUT_DATA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutData, &[], classify_data_mode);

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

pub(crate) fn execute_put_data_l16_bytes(
    image: &mut DynamicImage,
    data: &[u8],
    entry_count: usize,
    scale: f64,
    offset: f64,
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
    match (selection.implementation.mode, image) {
        (Mode::ImageI16, DynamicImage::ImageI16(image)) => {
            super::i16::default::put_data_impl::execute_l16_bytes(
                image,
                data,
                entry_count,
                scale,
                offset,
            )
        }
        (Mode::ImageI16L, DynamicImage::ImageI16L(image)) => {
            super::i16l::default::put_data_impl::execute_l16_bytes(
                image,
                data,
                entry_count,
                scale,
                offset,
            )
        }
        (Mode::ImageI16B, DynamicImage::ImageI16B(image)) => {
            super::i16b::default::put_data_impl::execute_l16_bytes(
                image,
                data,
                entry_count,
                scale,
                offset,
            )
        }
        (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => {
            super::i16n::default::put_data_impl::execute_l16_bytes(
                image,
                data,
                entry_count,
                scale,
                offset,
            )
        }
        _ => Err(PilError::InternalError(
            "putdata I;16 storage mismatch".into(),
        )),
    }
}

pub(crate) static PUT_DATA_VALUE_AT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutDataValueAt,
    &[],
    classify_data_value_at_mode,
);

pub(crate) static PUT_PIXEL_VALUE_AT_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::PutPixelValueAt,
    &[],
    classify_pixel_value_at_mode,
);

const PUT_BAND_SINGLE_INPUTS: &[InputContract] = &[
    InputContract::CallerValidatedBandIndex,
    InputContract::CallerValidatedSameModeSameSizeBandImage,
];
const PUT_BAND_MULTI_INPUTS: &[InputContract] = &[
    InputContract::CallerValidatedBandIndex,
    InputContract::CallerValidatedSameSizeByteSingleBandImage,
];
const FILL_BAND_INPUTS: &[InputContract] = &[InputContract::CallerValidatedBandIndex];
fn classify_put_band_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => {
            ModeSupport::serial(OperationId::PutBand, mode, PUT_BAND_SINGLE_INPUTS)
        }
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ModeSupport::serial(OperationId::PutBand, mode, PUT_BAND_MULTI_INPUTS),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
    }
}

fn classify_fill_band_mode(mode: Mode) -> ModeSupport {
    if super::helpers::imaging_core_band::is_byte_mode(mode) {
        ModeSupport::serial(OperationId::FillBand, mode, FILL_BAND_INPUTS)
    } else {
        ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        }
    }
}

pub(crate) static PUT_BAND_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutBand, &[], classify_put_band_mode);

pub(crate) static FILL_BAND_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::FillBand, &[], classify_fill_band_mode);

pub(crate) fn select_put_band(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    PUT_BAND_SUPPORT.for_mode(mode).select_serial(context)
}

pub(crate) fn select_fill_band(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    FILL_BAND_SUPPORT.for_mode(mode).select_serial(context)
}

pub(crate) fn execute_put_band(
    destination: &mut DynamicImage,
    source: &DynamicImage,
    band: usize,
    selection: StrategySelection,
) -> Result<(), PilError> {
    validate_band_selection(selection, OperationId::PutBand, destination.mode())?;
    super::helpers::imaging_core_band::put_band(destination, source, band)
}

pub(crate) fn execute_fill_band(
    image: &mut DynamicImage,
    band: usize,
    value: u8,
    selection: StrategySelection,
) -> Result<(), PilError> {
    validate_band_selection(selection, OperationId::FillBand, image.mode())?;
    super::helpers::imaging_core_band::fill_band(image, band, value)
}

fn validate_band_selection(
    selection: StrategySelection,
    operation: OperationId,
    mode: Mode,
) -> Result<(), PilError> {
    if selection.implementation.operation != operation
        || selection.implementation.mode != mode
        || selection.implementation.architecture != crate::compute::Architecture::Default
    {
        return Err(PilError::InternalError(
            "ImagingCore band strategy did not match its raster".into(),
        ));
    }
    Ok(())
}

pub(crate) fn select_put_pixel_value_at(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    PUT_PIXEL_VALUE_AT_SUPPORT
        .for_mode(mode)
        .select_serial(context)
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
    match (mode, image, input) {
        (
            Mode::ImageI32,
            DynamicImage::ImageI32(image),
            PutPixelValueAtInput::NumericBytes(bytes),
        ) => super::i32::default::put_pixel_impl::execute_value_at(image, pixel_index, bytes),
        (
            Mode::ImageF32,
            DynamicImage::ImageF32(image),
            PutPixelValueAtInput::NumericBytes(bytes),
        ) => super::f32::default::put_pixel_impl::execute_value_at(image, pixel_index, bytes),
        (Mode::ImageI16, DynamicImage::ImageI16(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16L, DynamicImage::ImageI16L(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16l::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16B, DynamicImage::ImageI16B(image), PutPixelValueAtInput::Luma16(sample)) => {
            super::i16b::default::put_pixel_impl::execute_value_at(image, pixel_index, sample)
        }
        (Mode::ImageI16N, DynamicImage::ImageI16N(image), PutPixelValueAtInput::Luma16(sample)) => {
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
    PUT_DATA_VALUE_AT_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

pub(crate) fn execute_put_data_value_at(
    image: &mut DynamicImage,
    pixel_index: usize,
    input: super::helpers::put_data_value_at::ValueAtInput,
    selection: StrategySelection,
) -> Result<(), PilError> {
    if selection.implementation.operation != OperationId::PutDataValueAt {
        return Err(PilError::InternalError(
            "putdata value-at strategy does not match its raster mode".into(),
        ));
    }
    if selection.implementation.architecture != crate::compute::Architecture::Default {
        return Err(PilError::InternalError(
            "unbound putdata value-at architecture".into(),
        ));
    }
    let mode = selection.implementation.mode;
    if is_luma16_mode(mode) {
        super::value_at::execute_luma16_value_at(mode, image, pixel_index, input)
    } else {
        super::value_at::execute_byte_value_at(mode, image, pixel_index, input)
    }
}

fn is_luma16_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    )
}

pub(crate) static PUT_ALPHA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutAlpha, &[], classify_alpha_mode);
pub(crate) static PUT_ALPHA_DATA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutAlphaData, &[], classify_alpha_data_mode);

pub(crate) fn prepare_alpha_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
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

pub(crate) fn prepare_alpha_data_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
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

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
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

/// Execute one prepared CPU PutPixel write for GPU host-control calculations.
pub(crate) fn execute_put_pixel(
    image: &DynamicImage,
    write: PixelWrite,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    prepare_put_pixel(image, write, context)?.execute()
}

pub(crate) static PUT_PIXEL_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutPixel, &[], classify_pixel_mode);
pub(crate) static PUT_PIXEL_BATCH_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PutPixelBatch, &[], classify_pixel_batch_mode);

pub(crate) fn prepare_pixel_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutPixel { x, y, color, .. } = op else {
        return Err(PilError::ValueError("expected PutPixel op".into()));
    };
    prepare_put_pixel(
        input.image,
        PixelWrite {
            x: *x,
            y: *y,
            color: *color,
        },
        ctx,
    )
    .map(crate::compute::registry::PreparedOperation::from_put_pixel)
}

pub(crate) fn prepare_data_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::PutData {
        data,
        hidden_lanes,
        payload_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    prepare_put_data(input.image, data, *payload_mode, hidden_lanes.clone(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_put_data)
}
