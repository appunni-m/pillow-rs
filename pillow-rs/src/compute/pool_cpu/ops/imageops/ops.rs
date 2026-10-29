//! ImageOps support contracts and prepared mode/architecture dispatch.
//!
//! TODO(pillow-rs#3ptb): retire Expand's separate SIMD adapter after eager,
//! fusion, and compatibility routes use its prepared CPU architecture plan.
//! Registry CPU execution now uses the exhaustive prepared native-mode plan.
//! TODO(pillow-rs#3ptb): route remaining Colorize eager, fusion, and
//! compatibility callers through its prepared CPU strategy, then remove the
//! independent `simd_colorize` registration. Separate SIMD adapters for
//! Autocontrast, Equalize, CropBorder, Invert, Posterize, and Solarize remain
//! registered until eager, fusion, and compatibility routes use their prepared
//! CPU strategies. Autocontrast and Equalize kernels are now declared under
//! their exact mode and architecture owners.
//! Posterize/Solarize vector kernels are now declared for native L/RGB modes;
//! the independent adapter routes still need reconciliation and retirement.

use crate::compute::{
    Architecture,
    ExecutionContext,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
        OperationSupport, OutputModeRule, PaletteKnowledge, RequestFacts, RejectionKind,
        StrategyGuard, StrategyInputs, StrategySelection, SupportAssessment,
    },
};
use crate::compute::pool_cpu::ops::crop::ops::{CropRequest, PreparedCrop};
use crate::compute::pool_cpu::ops::crop::helpers::strategy::luma16_copy_architectures;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use std::borrow::Cow;

pub(crate) use super::autocontrast::PreparedAutocontrast;
pub(crate) use super::equalize::PreparedEqualize;
pub(crate) use super::expand::{
    PreparedExpand, SUPPORT as EXPAND_SUPPORT, prepare_pipeline as prepare_expand_pipeline,
};

const CROP_BORDER_INPUTS: &[InputContract] = &[InputContract::CropBorder];

static CROP_BORDER_I16_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_copy_architectures(
        OperationId::CropBorder,
        Mode::ImageI16,
        StrategyGuard::NativeCropBorderBytes { channels: 2 },
    );
static CROP_BORDER_I16L_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_copy_architectures(
        OperationId::CropBorder,
        Mode::ImageI16L,
        StrategyGuard::NativeCropBorderBytes { channels: 2 },
    );
static CROP_BORDER_I16B_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_copy_architectures(
        OperationId::CropBorder,
        Mode::ImageI16B,
        StrategyGuard::NativeCropBorderBytes { channels: 2 },
    );
static CROP_BORDER_I16N_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_copy_architectures(
        OperationId::CropBorder,
        Mode::ImageI16N,
        StrategyGuard::NativeCropBorderBytes { channels: 2 },
    );

fn crop_border_luma16(
    mode: Mode,
    architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::CropBorder,
        mode,
        OutputModeRule::Preserve,
        CROP_BORDER_INPUTS,
        architectures,
    )
}

// NO_PILLOW_SUPPORT: CropBorder also preserves the five internal high-depth modes.
macro_rules! declare_crop_border_modes {
    ($($mode:ident),+ $(,)?) => {
        fn classify_crop_border_mode(mode: Mode) -> ModeSupport {
            match mode {
                Mode::ImageI16 => crop_border_luma16(mode, &CROP_BORDER_I16_ARCHITECTURES),
                Mode::ImageI16L => crop_border_luma16(mode, &CROP_BORDER_I16L_ARCHITECTURES),
                Mode::ImageI16B => crop_border_luma16(mode, &CROP_BORDER_I16B_ARCHITECTURES),
                Mode::ImageI16N => crop_border_luma16(mode, &CROP_BORDER_I16N_ARCHITECTURES),
                $(Mode::$mode => ModeSupport::serial(
                    OperationId::CropBorder,
                    Mode::$mode,
                    CROP_BORDER_INPUTS,
                ),)+
            }
        }
    };
}

declare_crop_border_modes! {
    One,
    ImageLuma8,
    ImageLumaA8,
    ImageLaPremultiplied8,
    ImagePalette8,
    ImagePaletteAlpha8,
    ImageRgb8,
    ImageRgba8,
    ImageRgbPremultiplied8,
    ImageRgbx8,
    ImageCmyk8,
    ImageYCbCr8,
    ImageLab8,
    ImageHsv8,
    ImageI32,
    ImageF32,
    ImageLumaA16,
    ImageRgb16,
    ImageRgba16,
    ImageRgb32F,
    ImageRgba32F,
}

/// Complete mode contract for equal-border crop, including valid empty output.
pub(crate) static CROP_BORDER_SUPPORT: OperationSupport =
    OperationSupport::new(
        OperationId::CropBorder,
        CROP_BORDER_INPUTS,
        classify_crop_border_mode,
    );

/// Prepared CropBorder plan composed from the exact-mode crop strategy.
pub(crate) enum PreparedCropBorder<'a> {
    Crop(PreparedCrop<'a>, StrategySelection),
    Empty(&'a DynamicImage, u32, StrategySelection),
}

impl PreparedCropBorder<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Crop(_, selection) | Self::Empty(_, _, selection) => selection,
        }
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self {
            Self::Crop(crop, _) => crop.execute(),
            Self::Empty(image, border, _) => {
                let (width, height) = image.dimensions();
                Ok(image.crop_imm(border, border, width - 2 * border, height - 2 * border))
            }
        }
    }
}

pub(crate) fn prepare_crop_border<'a>(
    image: &'a DynamicImage,
    border: u32,
    ctx: &ExecutionContext,
) -> Result<PreparedCropBorder<'a>, PilError> {
    let (width, height) = image.dimensions();
    let assessment = CROP_BORDER_SUPPORT.for_mode(image.mode()).assess(
        ctx,
        StrategyInputs {
            width,
            height,
            byte_len: Some(image.as_bytes().len()),
            palette: PaletteKnowledge::Unknown,
            request: RequestFacts::CropBorder { border },
        },
    )?;
    let selection = match assessment {
        SupportAssessment::Ready(selection) => selection,
        SupportAssessment::Rejected { reason, kind, .. } => return Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => return Err(PilError::InternalError(
            "CropBorder preparation has unresolved input facts".into(),
        )),
    };
    let crop_width = width - 2 * border;
    let crop_height = height - 2 * border;
    if crop_width == 0 || crop_height == 0 {
        return Ok(PreparedCropBorder::Empty(image, border, selection));
    }
    let crop = crate::compute::pool_cpu::ops::crop::ops::prepare(
        image,
        CropRequest {
            left: border,
            top: border,
            right: width - border,
            bottom: height - border,
        },
        ctx,
    )?;
    // CropBorder owns a matching I;16 architecture declaration; Crop owns the
    // actual prepared row-copy kernel selected under its own operation id.
    Ok(PreparedCropBorder::Crop(crop, selection))
}

pub(crate) fn prepare_crop_border_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::CropBorder { border } = op else {
        return Err(PilError::ValueError("expected CropBorder op".into()));
    };
    prepare_crop_border(input.image, *border, ctx)
        .map(crate::compute::registry::PreparedOperation::from_crop_border)
}

macro_rules! declare_invert_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        pub(crate) enum PreparedInvert<'a> {
            $($mode(super::$module::ops::Strategy<'a>),)+
        }

        fn classify_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::SUPPORT,)+
                Mode::ImageI32 | Mode::ImageF32
                | Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N =>
                    rejected(mode, "invert L buffer shape mismatch"),
                Mode::ImageLumaA16 => rejected(mode, "invert LA buffer shape mismatch"),
                Mode::ImageRgb16 | Mode::ImageRgb32F =>
                    rejected(mode, "invert RGB buffer shape mismatch"),
                Mode::ImageRgba16 | Mode::ImageRgba32F =>
                    rejected(mode, "invert RGBA buffer shape mismatch"),
            }
        }

        fn prepare_mode<'a>(
            image: &'a DynamicImage,
            ctx: &ExecutionContext,
        ) -> Result<PreparedInvert<'a>, PilError> {
            match image {
                $(DynamicImage::$mode(image) =>
                    super::$module::ops::prepare(image, ctx).map(PreparedInvert::$mode),)+
                DynamicImage::ImageI32(_) | DynamicImage::ImageF32(_)
                | DynamicImage::ImageI16(_) | DynamicImage::ImageI16L(_)
                | DynamicImage::ImageI16B(_) | DynamicImage::ImageI16N(_)
                | DynamicImage::ImageLumaA16(_) => Err(rejection_error(image.mode())),
                DynamicImage::ImageRgb16(_) | DynamicImage::ImageRgb32F(_)
                | DynamicImage::ImageRgba16(_) | DynamicImage::ImageRgba32F(_) =>
                    Err(rejection_error(image.mode())),
            }
        }

        impl PreparedInvert<'_> {
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+
                }
            }
        }
    };
}

declare_invert_modes! {
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
}

fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected { mode, reason, kind: RejectionKind::InternalError }
}

fn rejection_error(mode: Mode) -> PilError {
    match INVERT_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => {
            PilError::InternalError("Invert support and mode dispatch disagree".into())
        }
    }
}

/// Complete support contract for Pillow's bytewise Invert operation.
pub(crate) static INVERT_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Invert, &[], classify_mode);

const POSTERIZE_INPUTS: &[InputContract] = &[InputContract::PosterizeBits];
const POSTERIZE_LUMA_INPUTS: &[InputContract] = &[
    InputContract::PosterizeBits,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
];
const POSTERIZE_RGB_INPUTS: &[InputContract] = &[
    InputContract::PosterizeBits,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
];
const SOLARIZE_INPUTS: &[InputContract] = &[];
const SOLARIZE_LUMA_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
];
const SOLARIZE_RGB_INPUTS: &[InputContract] = &[
    InputContract::CheckedByteDimensions { bytes_per_pixel: 3, allow_empty: true },
];

fn classify_native_byte_point_mode(
    operation: OperationId,
    mode: Mode,
    luma_inputs: &'static [InputContract],
    rgb_inputs: &'static [InputContract],
    luma_architectures: &'static [ImplementationSupport],
    rgb_architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            luma_inputs,
            luma_architectures,
        ),
        Mode::ImageRgb8 => ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            rgb_inputs,
            rgb_architectures,
        ),
        // TODO(pillow-rs#3ptb): add palette-index LUT strategies once its
        // palette/identity contract is migrated; preserve the current public
        // NotImplementedError until that work is complete.
        Mode::ImagePalette8 => ModeSupport::Rejected {
            mode,
            reason: "mode P support coming soon",
            kind: RejectionKind::NotImplementedError,
        },
        Mode::One
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => rejected_native_byte_point_mode(mode),
    }
}

const POSTERIZE_LUMA_ARCHITECTURES: [ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Posterize,
        Mode::ImageLuma8,
        1,
    );
const POSTERIZE_RGB_ARCHITECTURES: [ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Posterize,
        Mode::ImageRgb8,
        3,
    );
const SOLARIZE_LUMA_ARCHITECTURES: [ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Solarize,
        Mode::ImageLuma8,
        1,
    );
const SOLARIZE_RGB_ARCHITECTURES: [ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Solarize,
        Mode::ImageRgb8,
        3,
    );
const COLORIZE_LUMA_ARCHITECTURES: [ImplementationSupport; 3] =
    super::helpers::strategy::native_byte_point_architectures(
        OperationId::Colorize,
        Mode::ImageLuma8,
        1,
    );

fn rejected_native_byte_point_mode(mode: Mode) -> ModeSupport {
    let reason = match mode {
        Mode::One => "not supported for mode 1",
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8 =>
            "not supported for this mode",
        Mode::ImageLumaA8 => "not supported for mode LA",
        Mode::ImageLaPremultiplied8 => "not supported for mode La",
        Mode::ImagePaletteAlpha8 => "not supported for mode PA",
        Mode::ImageRgba8 => "not supported for mode RGBA",
        Mode::ImageRgbPremultiplied8 => "not supported for mode RGBa",
        Mode::ImageRgbx8 => "not supported for mode RGBX",
        Mode::ImageCmyk8 => "not supported for mode CMYK",
        Mode::ImageYCbCr8 => "not supported for mode YCbCr",
        Mode::ImageLab8 => "not supported for mode LAB",
        Mode::ImageHsv8 => "not supported for mode HSV",
        Mode::ImageI32 => "not supported for mode I",
        Mode::ImageF32 => "not supported for mode F",
        Mode::ImageI16 => "not supported for mode I;16",
        Mode::ImageI16L => "not supported for mode I;16L",
        Mode::ImageI16B => "not supported for mode I;16B",
        Mode::ImageI16N => "not supported for mode I;16N",
        Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16
        | Mode::ImageRgb32F | Mode::ImageRgba32F => "not supported for internal mode",
    };
    ModeSupport::Rejected { mode, reason, kind: RejectionKind::OsError }
}

fn classify_posterize_mode(mode: Mode) -> ModeSupport {
    classify_native_byte_point_mode(
        OperationId::Posterize,
        mode,
        POSTERIZE_LUMA_INPUTS,
        POSTERIZE_RGB_INPUTS,
        &POSTERIZE_LUMA_ARCHITECTURES,
        &POSTERIZE_RGB_ARCHITECTURES,
    )
}

/// Exhaustive native-mode and parameter contract for Pillow Posterize.
pub(crate) static POSTERIZE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Posterize, POSTERIZE_INPUTS, classify_posterize_mode);

fn classify_solarize_mode(mode: Mode) -> ModeSupport {
    classify_native_byte_point_mode(
        OperationId::Solarize,
        mode,
        SOLARIZE_LUMA_INPUTS,
        SOLARIZE_RGB_INPUTS,
        &SOLARIZE_LUMA_ARCHITECTURES,
        &SOLARIZE_RGB_ARCHITECTURES,
    )
}

/// Exhaustive native-mode contract for Pillow Solarize.
pub(crate) static SOLARIZE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Solarize, SOLARIZE_INPUTS, classify_solarize_mode);

macro_rules! declare_colorize_modes {
    ($($mode:ident),+ $(,)?) => {
        fn classify_colorize_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => ModeSupport::cpu(
                    OperationId::Colorize,
                    Mode::$mode,
                    OutputModeRule::Fixed(Mode::ImageRgb8),
                    &[],
                    colorize_architectures(Mode::$mode),
                ),)+
            }
        }
    };
}

fn colorize_architectures(mode: Mode) -> &'static [ImplementationSupport] {
    match mode {
        Mode::ImageLuma8 => &COLORIZE_LUMA_ARCHITECTURES,
        _ => &[],
    }
}

declare_colorize_modes! {
    One,
    ImageLuma8,
    ImageLumaA8,
    ImageLaPremultiplied8,
    ImagePalette8,
    ImagePaletteAlpha8,
    ImageRgb8,
    ImageRgba8,
    ImageRgbPremultiplied8,
    ImageRgbx8,
    ImageCmyk8,
    ImageYCbCr8,
    ImageLab8,
    ImageHsv8,
    ImageI32,
    ImageF32,
    ImageI16,
    ImageI16L,
    ImageI16B,
    ImageI16N,
    ImageLumaA16,
    ImageRgb16,
    ImageRgba16,
    ImageRgb32F,
    ImageRgba32F,
}

/// Complete source-mode and fixed-output contract for Pillow Colorize.
// NO_PILLOW_SUPPORT: Colorize's typed strategies also accept the five internal
// high-depth modes and explicitly produce ImageRgb8.
pub(crate) static COLORIZE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Colorize, &[], classify_colorize_mode);

/// Prepared Colorize input, LUT, and exact source-mode selection.
pub(crate) struct PreparedColorize<'a> {
    source: Cow<'a, [u8]>,
    width: u32,
    height: u32,
    lut: [[u8; 256]; 3],
    selection: StrategySelection,
}

impl PreparedColorize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mode = self.selection.implementation.mode;
        let architecture = self.selection.implementation.architecture;
        match (mode, architecture) {
            (Mode::ImageLuma8, Architecture::Default) => {
                super::luma8::default::colorize_impl::execute(
                    self.source.as_ref(), self.width, self.height, &self.lut,
                )
            }
            (Mode::ImageLuma8, Architecture::Sse2) => super::luma8::sse2::colorize_impl::execute(
                self.source.as_ref(), self.width, self.height, &self.lut,
            ),
            (Mode::ImageLuma8, Architecture::Neon) => super::luma8::neon::colorize_impl::execute(
                self.source.as_ref(), self.width, self.height, &self.lut,
            ),
            (Mode::ImageLuma8, Architecture::WasmSimd128) => {
                super::luma8::wasm_simd128::colorize_impl::execute(
                    self.source.as_ref(), self.width, self.height, &self.lut,
                )
            }
            (_, Architecture::Default) => super::helpers::colorize::execute_default(
                self.source.as_ref(), self.width, self.height, &self.lut,
            ),
            _ => Err(PilError::InternalError(
                "colorize selection has no matching CPU kernel".into(),
            )),
        }
    }
}

fn prepare_colorize<'a>(
    image: &'a DynamicImage,
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
    ctx: &ExecutionContext,
) -> Result<PreparedColorize<'a>, PilError> {
    let selection = mode_selection(&COLORIZE_SUPPORT, image, RequestFacts::Unspecified, ctx)?;
    let source = match image {
        DynamicImage::ImageLuma8(gray) => Cow::Borrowed(gray.as_raw()),
        _ => Cow::Owned(image.to_luma8().into_raw()),
    };
    let lut = super::helpers::colorize::lookup_tables(
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    );
    Ok(PreparedColorize {
        source,
        width: image.width(),
        height: image.height(),
        lut,
        selection,
    })
}

pub(crate) fn prepare_colorize_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    else {
        return Err(PilError::ValueError("expected Colorize op".into()));
    };
    prepare_colorize(
        input.image,
        black,
        white,
        *mid,
        *blackpoint,
        *midpoint,
        *whitepoint,
        ctx,
    )
    .map(crate::compute::registry::PreparedOperation::from_colorize)
}

enum NativeBytePointSource<'a> {
    Luma(&'a crate::raster::GrayImage),
    Rgb(&'a crate::raster::RgbImage),
}

#[derive(Clone, Copy)]
enum PointArchitecture {
    Default,
    Sse2,
    Neon,
    WasmSimd128,
}

fn point_architecture(selection: &StrategySelection) -> Result<PointArchitecture, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => Ok(PointArchitecture::Default),
        Architecture::Sse2 => Ok(PointArchitecture::Sse2),
        Architecture::Neon => Ok(PointArchitecture::Neon),
        Architecture::WasmSimd128 => Ok(PointArchitecture::WasmSimd128),
        _ => Err(PilError::InternalError(
            "native-byte point selection has no matching CPU kernel".into(),
        )),
    }
}

/// A validated Posterize mode strategy and its selected CPU architecture.
pub(crate) struct PreparedPosterize<'a> {
    source: NativeBytePointSource<'a>,
    bits: u8,
    architecture: PointArchitecture,
    selection: StrategySelection,
}

impl PreparedPosterize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match (self.source, self.architecture) {
            (NativeBytePointSource::Luma(input), PointArchitecture::Default) => {
                super::luma8::default::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::Sse2) => {
                super::luma8::sse2::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::Neon) => {
                super::luma8::neon::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::WasmSimd128) => {
                super::luma8::wasm_simd128::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Default) => {
                super::rgb8::default::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Sse2) => {
                super::rgb8::sse2::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Neon) => {
                super::rgb8::neon::posterize_impl::execute(input, self.bits)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::WasmSimd128) => {
                super::rgb8::wasm_simd128::posterize_impl::execute(input, self.bits)
            }
        }
    }
}

/// A validated Solarize mode strategy and its selected CPU architecture.
pub(crate) struct PreparedSolarize<'a> {
    source: NativeBytePointSource<'a>,
    threshold: u8,
    architecture: PointArchitecture,
    selection: StrategySelection,
}

impl PreparedSolarize<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match (self.source, self.architecture) {
            (NativeBytePointSource::Luma(input), PointArchitecture::Default) => {
                super::luma8::default::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::Sse2) => {
                super::luma8::sse2::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::Neon) => {
                super::luma8::neon::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Luma(input), PointArchitecture::WasmSimd128) => {
                super::luma8::wasm_simd128::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Default) => {
                super::rgb8::default::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Sse2) => {
                super::rgb8::sse2::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::Neon) => {
                super::rgb8::neon::solarize_impl::execute(input, self.threshold)
            }
            (NativeBytePointSource::Rgb(input), PointArchitecture::WasmSimd128) => {
                super::rgb8::wasm_simd128::solarize_impl::execute(input, self.threshold)
            }
        }
    }
}

fn mode_selection(
    support: &'static OperationSupport,
    image: &DynamicImage,
    request: RequestFacts,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    super::helpers::strategy::select_mode(
        support.for_mode(image.mode()),
        image,
        request,
        PaletteKnowledge::Unknown,
        ctx,
    )
}

fn native_byte_point_source(image: &DynamicImage) -> Result<NativeBytePointSource<'_>, PilError> {
    match image {
        DynamicImage::ImageLuma8(input) => Ok(NativeBytePointSource::Luma(input)),
        DynamicImage::ImageRgb8(input) => Ok(NativeBytePointSource::Rgb(input)),
        _ => Err(PilError::InternalError(
            "native-byte point support and mode dispatch disagree".into(),
        )),
    }
}

fn prepare_posterize(
    image: &DynamicImage,
    bits: u8,
    ctx: &ExecutionContext,
) -> Result<PreparedPosterize<'_>, PilError> {
    let selection = mode_selection(
        &POSTERIZE_SUPPORT,
        image,
        RequestFacts::Posterize { bits },
        ctx,
    )?;
    let architecture = point_architecture(&selection)?;
    Ok(PreparedPosterize {
        source: native_byte_point_source(image)?,
        bits,
        architecture,
        selection,
    })
}

fn prepare_solarize(
    image: &DynamicImage,
    threshold: u8,
    ctx: &ExecutionContext,
) -> Result<PreparedSolarize<'_>, PilError> {
    let selection = mode_selection(&SOLARIZE_SUPPORT, image, RequestFacts::Unspecified, ctx)?;
    let architecture = point_architecture(&selection)?;
    Ok(PreparedSolarize {
        source: native_byte_point_source(image)?,
        threshold,
        architecture,
        selection,
    })
}

pub(crate) fn prepare_posterize_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Posterize { bits } = op else {
        return Err(PilError::ValueError("expected Posterize op".into()));
    };
    prepare_posterize(input.image, *bits, ctx)
        .map(crate::compute::registry::PreparedOperation::from_posterize)
}

pub(crate) fn prepare_solarize_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Solarize { threshold } = op else {
        return Err(PilError::ValueError("expected Solarize op".into()));
    };
    prepare_solarize(input.image, *threshold, ctx)
        .map(crate::compute::registry::PreparedOperation::from_solarize)
}

pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    ctx: &ExecutionContext,
) -> Result<PreparedInvert<'a>, PilError> {
    prepare_mode(image, ctx)
}

pub(crate) fn prepare_invert_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    if !matches!(op, PipelineOp::Invert) {
        return Err(PilError::ValueError("expected Invert op".into()));
    }
    prepare(input.image, ctx).map(crate::compute::registry::PreparedOperation::from_invert)
}

