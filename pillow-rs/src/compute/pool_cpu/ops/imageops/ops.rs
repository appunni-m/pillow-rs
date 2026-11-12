//! ImageOps support contracts and family-level prepared operation routing.
//!
//! Wrappers compose exact-mode plans; batch-fused Point uses Eval.
//! Point operations and Expand compose omitted physical lanes.
//! TODO(pillow-rs#3ptb): freeze raw-byte follow-on Paste parity for point and Expand outputs.

use crate::raster::GenericImageView;

use crate::compute::pool_cpu::ops::crop::ops::{CropRequest, PreparedCrop};
use crate::compute::{
    ExecutionContext,
    registry::support::{
        InputContract, ModeSupport, OperationId, OperationInputs, OperationModeRoute,
        OperationRouteOutcome, OperationRouteStage, OperationSupport, OutputModeRule,
        PaletteKnowledge, RejectionKind, RequestFacts, StrategyInputs, StrategySelection,
        SupportAssessment,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::helpers::expand::PreparedExpand;
pub(crate) use super::helpers::autocontrast::PreparedAutocontrast;
pub(crate) use super::helpers::equalize::PreparedEqualize;
pub(crate) use super::helpers::{
    IMAGEOPS_COLORIZE_CHILDREN, IMAGEOPS_COLORIZE_LUT_CHILDREN, IMAGEOPS_INVERT_CHILDREN,
    IMAGEOPS_LUT_CHILDREN,
};
use super::helpers::wrapper_routes::{
    classify_imageops_colorize_lut_route, classify_imageops_colorize_route,
    classify_imageops_invert_route, classify_imageops_posterize_route,
    classify_imageops_solarize_route,
};
use super::helpers::expand::{
    prepare as prepare_expand, prepare_with_native_fill,
};
pub(crate) use super::luma8::ops::PreparedColorize;

// Family-level support declarations are owned by the ImageOps entrypoint.
const AUTOCONTRAST_INPUTS: &[InputContract] = &[
    InputContract::AutocontrastParameters,
    InputContract::ImageOpsLumaMask,
];

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_autocontrast_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::AUTOCONTRAST_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::AUTOCONTRAST_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::AUTOCONTRAST_SUPPORT,
        _ => ModeSupport::Rejected {
            mode,
            reason: super::helpers::strategy::unsupported_mode_reason(mode),
            kind: RejectionKind::OsError,
        },
    }
}

pub(crate) static AUTOCONTRAST_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Autocontrast,
    AUTOCONTRAST_INPUTS,
    classify_autocontrast_mode,
);

const EQUALIZE_INPUTS: &[InputContract] = &[
    InputContract::ImageOpsLumaMask,
    InputContract::PaletteMetadata,
];

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_equalize_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => super::luma8::ops::EQUALIZE_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::EQUALIZE_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::EQUALIZE_SUPPORT,
        _ => ModeSupport::Rejected {
            mode,
            reason: super::helpers::strategy::unsupported_mode_reason(mode),
            kind: RejectionKind::OsError,
        },
    }
}

pub(crate) static EQUALIZE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Equalize,
    EQUALIZE_INPUTS,
    classify_equalize_mode,
);

/// Complete exact-mode support for native ImageOps.expand.
pub(crate) static EXPAND_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Expand,
    &[],
    classify_expand_mode,
);

macro_rules! declare_expand_mode_contract {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
        fn classify_expand_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::expand::SUPPORT,)+
            }
        }
    };
}

declare_expand_mode_contract! {
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

/// Exact-mode support for nearest-edge expansion on the callback handle.
pub(crate) static IMAGING_CORE_EXPAND_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ImagingCoreExpand,
    &[],
    super::helpers::expand_edges::classify_mode,
);

const QUANTIZE_ALLOWED_ALPHA_METHODS: &[u32] = &[2, 3];
const QUANTIZE_RECOGNIZED_METHODS: &[u32] = &[0, 1, 2, 3];
const QUANTIZE_METHOD_RESTRICTED_MODES: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLumaA16,
    Mode::ImageRgba16,
    Mode::ImageRgba32F,
];
const QUANTIZE_PALETTE_SOURCES: &[Mode] = &[Mode::ImageRgb8, Mode::ImageLuma8];
const QUANTIZE_PALETTE_CARRIERS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
];
const QUANTIZE_UNAVAILABLE_METHODS: &[u32] = &[3];
pub(super) const QUANTIZE_INPUT: InputContract = InputContract::QuantizeParameters {
    min_colors: 1,
    max_colors: 256,
    recognized_methods: QUANTIZE_RECOGNIZED_METHODS,
    method_restricted_modes: QUANTIZE_METHOD_RESTRICTED_MODES,
    allowed_methods: QUANTIZE_ALLOWED_ALPHA_METHODS,
    palette_mode: Mode::ImagePalette8,
    palette_sources: QUANTIZE_PALETTE_SOURCES,
    palette_carriers: QUANTIZE_PALETTE_CARRIERS,
    palette_carrier_rejection: "conversion not supported",
    unavailable_methods: QUANTIZE_UNAVAILABLE_METHODS,
    kmeans_nonnegative: true,
    colors_rejection: "bad number of colors",
    invalid_method_rejection: "quantization error",
    pillow_rgba_method_rejection: "Fast Octree (method == 2) and libimagequant (method == 3) \
         are the only valid methods for quantizing RGBA images",
    alpha_method_rejection: "only Fast Octree (method == 2) is supported for alpha-bearing CoreOnly modes",
    palette_mode_rejection: "bad mode for palette image",
    palette_source_rejection: "only RGB or L mode images can be quantized to a palette",
    unavailable_method_rejection: "dependency required by this method was not enabled at compile time",
    kmeans_rejection: "kmeans must not be negative",
};
const QUANTIZE_INPUTS: &[InputContract] = &[InputContract::PaletteMetadata, QUANTIZE_INPUT];
const QUANTIZE_PALETTED_OUTPUT: OutputModeRule = OutputModeRule::Fixed(Mode::ImagePalette8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QuantizeSource {
    Luma,
    Palette,
    Rgba,
    RgbPremultiplied,
    Rgb,
    Rgbx,
    Native,
}

#[derive(Clone, Copy)]
enum QuantizeSupportScope {
    Pillow,
    CoreOnly,
}

#[derive(Clone, Copy)]
pub(super) struct QuantizeModeStrategy {
    pub(super) support: ModeSupport,
    pub(super) source: QuantizeSource,
    pub(super) default_method: u32,
}

/// Binds each exact mode's support declaration to its concrete source strategy.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(super) fn quantize_mode_strategy(mode: Mode) -> QuantizeModeStrategy {
    let (source, scope, default_method) = match mode {
        Mode::ImageLuma8 => (QuantizeSource::Luma, QuantizeSupportScope::Pillow, 0),
        Mode::ImagePalette8 => (QuantizeSource::Palette, QuantizeSupportScope::Pillow, 0),
        Mode::ImageRgba8 => (QuantizeSource::Rgba, QuantizeSupportScope::Pillow, 2),
        Mode::ImageRgbPremultiplied8 => (
            QuantizeSource::RgbPremultiplied,
            QuantizeSupportScope::CoreOnly,
            2,
        ),
        Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16
        | Mode::ImageRgba16
        | Mode::ImageRgba32F => (QuantizeSource::Native, QuantizeSupportScope::CoreOnly, 2),
        Mode::ImageRgbx8 => (QuantizeSource::Rgbx, QuantizeSupportScope::CoreOnly, 0),
        Mode::ImageRgb8 => (QuantizeSource::Rgb, QuantizeSupportScope::Pillow, 0),
        // NO_PILLOW_SUPPORT: these exact mode strategies extend Pillow's
        // Quantize source contract without changing the Pillow-defined routes.
        Mode::One
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
        | Mode::ImageRgb16
        | Mode::ImageRgb32F => (QuantizeSource::Native, QuantizeSupportScope::CoreOnly, 0),
    };
    let support = match scope {
        QuantizeSupportScope::CoreOnly => ModeSupport::cpu_core_only(
            OperationId::Quantize,
            mode,
            QUANTIZE_PALETTED_OUTPUT,
            QUANTIZE_INPUTS,
            &[],
            "CoreOnly mode strategies extend Pillow's Quantize mode contract",
        ),
        QuantizeSupportScope::Pillow => ModeSupport::cpu(
            OperationId::Quantize,
            mode,
            QUANTIZE_PALETTED_OUTPUT,
            QUANTIZE_INPUTS,
            &[],
        ),
    };
    QuantizeModeStrategy {
        support,
        source,
        default_method,
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_quantize_mode(mode: Mode) -> ModeSupport {
    quantize_mode_strategy(mode).support
}

pub(crate) static QUANTIZE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Quantize,
    QUANTIZE_INPUTS,
    classify_quantize_mode,
);

pub(crate) use super::helpers::quantize::{
    default_method, execute_with_palette, prepare as prepare_quantize, validate_kmeans_count,
    validate_pillow_rgba_method,
};

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_autocontrast_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Autocontrast { options, mask } = op else {
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    };
    super::helpers::autocontrast::prepare(
        input.image,
        options,
        mask.as_ref(),
        &AUTOCONTRAST_SUPPORT,
        ctx,
    )
        .map(crate::compute::registry::PreparedOperation::from_autocontrast)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_equalize_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let mask = match op {
        PipelineOp::Equalize => None,
        PipelineOp::EqualizeMasked { mask } => Some(mask),
        _ => return Err(PilError::ValueError("expected Equalize op".into())),
    };
    super::helpers::equalize::prepare(input, mask, &EQUALIZE_SUPPORT, ctx)
        .map(crate::compute::registry::PreparedOperation::from_equalize)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_expand_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let image = input.image;
    let prepared = match op {
        PipelineOp::Expand { border, fill } => prepare_expand(image, *border, *fill, ctx),
        PipelineOp::ExpandTyped {
            border,
            fill,
            input: native_fill,
        } => prepare_with_native_fill(image, *border, *fill, *native_fill, ctx),
        _ => return Err(PilError::ValueError("expected Expand op".into())),
    }?;
    Ok(crate::compute::registry::PreparedOperation::from_expand(
        prepared,
    ))
}

pub(in crate::compute::pool_cpu::ops::imageops) const CROP_BORDER_INPUTS: &[InputContract] =
    &[InputContract::CropBorder];

macro_rules! declare_crop_border_modes {
    ($($mode:ident => $module:ident),+ $(,)?) => {
        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
        fn classify_crop_border_mode(mode: Mode) -> ModeSupport {
            match mode {
                $(Mode::$mode => super::$module::ops::CROP_BORDER_SUPPORT,)+
            }
        }
    };
}

declare_crop_border_modes! {
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

/// Complete mode contract for equal-border crop, including valid empty output.
pub(crate) static CROP_BORDER_SUPPORT: OperationSupport = OperationSupport::new(
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
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Crop(_, selection) | Self::Empty(_, _, selection) => selection,
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
        SupportAssessment::Deferred { .. } => {
            return Err(PilError::InternalError(
                "CropBorder preparation has unresolved input facts".into(),
            ));
        }
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
            // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
            pub(crate) fn selection(&self) -> &StrategySelection {
                match self { $(Self::$mode(strategy) => strategy.selection(),)+ }
            }

            // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
            pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
                let mut output = match self {
                    $(Self::$mode(strategy) => super::$module::ops::execute(strategy),)+
                }?;
                super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
                Ok(output)
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::InternalError,
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn rejection_error(mode: Mode) -> PilError {
    match INVERT_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => kind.into_error(reason),
        ModeSupport::Supported { .. } => {
            PilError::InternalError("Invert support and mode dispatch disagree".into())
        }
    }
}

/// Exact-mode support contract for ImageOps Invert and its CoreOnly routes.
pub(crate) static INVERT_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Invert, &[], classify_mode);

/// Public ImageOps wrapper contract; Python routes through public `Image.point`
/// and Eval, while the direct Rust API retains the exact-mode Invert child.
// TODO(pillow-rs#3ptb): Freeze P's NotImplementedError, Pillow's unsupported
// high-depth wrapper errors, the four immediate byte-mode rejections, direct
// Rust child-admission timing, and Python callback mode overrides in CPU
// parity, including route receipts.
pub(crate) static IMAGEOPS_INVERT_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsInvert,
    &[],
    classify_imageops_invert_route,
    IMAGEOPS_INVERT_CHILDREN,
);

// TODO(pillow-rs#3ptb): verify wrapper mode/fault order, custom mode and point
// callbacks, unsupported-mode formatting faults, and selected/fallback
// receipts for ImageOps Invert, Posterize, and Solarize. Their Rust-native
// operation contracts remain separate.

/// Wrapper-visible Posterize route through the shared ImageOps LUT adapter.
pub(crate) static IMAGEOPS_POSTERIZE_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsPosterize,
    &[],
    classify_imageops_posterize_route,
    IMAGEOPS_LUT_CHILDREN,
);

/// Wrapper-visible Solarize route through the shared ImageOps LUT adapter.
pub(crate) static IMAGEOPS_SOLARIZE_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsSolarize,
    &[],
    classify_imageops_solarize_route,
    IMAGEOPS_LUT_CHILDREN,
);

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn validate_imageops_invert_mode(mode: Mode) -> Result<(), PilError> {
    match IMAGEOPS_INVERT_SUPPORT.route_for_mode(mode) {
        OperationModeRoute::ComposedFallback {
            outcome:
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::WrapperGuard,
                    reason,
                    kind,
                },
            ..
        } => Err(kind.into_error(reason)),
        _ => Ok(()),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_posterize_mode(mode: Mode) -> ModeSupport {
    super::helpers::point_support::classify_native_byte_point_mode(
        mode,
        super::luma8::ops::POSTERIZE_SUPPORT,
        super::palette8::ops::POSTERIZE_SUPPORT,
        super::rgb8::ops::POSTERIZE_SUPPORT,
    )
}

const POSTERIZE_INPUTS: &[InputContract] = &[InputContract::PosterizeBits];

/// Exhaustive Posterize contract, including the core-only P index strategy.
pub(crate) static POSTERIZE_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Posterize,
    POSTERIZE_INPUTS,
    classify_posterize_mode,
);

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_solarize_mode(mode: Mode) -> ModeSupport {
    super::helpers::point_support::classify_native_byte_point_mode(
        mode,
        super::luma8::ops::SOLARIZE_SUPPORT,
        super::palette8::ops::SOLARIZE_SUPPORT,
        super::rgb8::ops::SOLARIZE_SUPPORT,
    )
}

/// Exhaustive Solarize contract, including the core-only P index strategy.
pub(crate) static SOLARIZE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Solarize, &[], classify_solarize_mode);

macro_rules! declare_colorize_modes {
    ($($mode:ident),+ $(,)?) => {
        // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
        fn classify_colorize_mode(mode: Mode) -> ModeSupport {
            match mode {
                Mode::ImageLuma8 => super::luma8::ops::COLORIZE_SUPPORT,
                $(Mode::$mode => rejected_colorize_mode(mode),)+
            }
        }
    };
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn rejected_colorize_mode(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: "mode must be L",
        kind: RejectionKind::ValueError,
    }
}

declare_colorize_modes! {
    One,
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

/// Pillow's exact `L` source mode and fixed-RGB output contract for Colorize.
pub(crate) static COLORIZE_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Colorize, &[], classify_colorize_mode)
        .with_rejected_output(OutputModeRule::Fixed(Mode::ImageRgb8));

/// Public ImageOps wrapper contract: L converts to RGB before the shared LUT.
// TODO(pillow-rs#3ptb): freeze subclass `convert()` results that return P or
// unsupported modes, including the post-conversion error and selection receipt.
pub(crate) static IMAGEOPS_COLORIZE_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsColorize,
    &[],
    classify_imageops_colorize_route,
    IMAGEOPS_COLORIZE_CHILDREN,
);

/// Support for Colorize's `_lut` stage after public Convert returns.
// TODO(pillow-rs#3ptb): verify mode overrides, output metadata, and selected
// or fallback Eval receipts after public Convert and Point callbacks.
pub(crate) static IMAGEOPS_COLORIZE_LUT_SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::ImageOpsColorizeLut,
    &[],
    classify_imageops_colorize_lut_route,
    IMAGEOPS_COLORIZE_LUT_CHILDREN,
);

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
    match image {
        DynamicImage::ImageLuma8(gray) => Ok(PreparedColorize::new(
            gray, *black, *white, mid, blackpoint, midpoint, whitepoint, selection,
        )),
        _ => Err(PilError::InternalError(
            "Colorize support and mode dispatch disagree".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

/// Prepared Posterize strategy routed to one exact mode owner.
pub(crate) enum PreparedPosterize<'a> {
    Luma(super::luma8::ops::PreparedPosterize<'a>),
    Palette(super::palette8::ops::PreparedPosterize<'a>),
    Rgb(super::rgb8::ops::PreparedPosterize<'a>),
}

impl PreparedPosterize<'_> {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self {
            Self::Luma(prepared) => prepared.execute()?,
            Self::Palette(prepared) => prepared.execute()?,
            Self::Rgb(prepared) => prepared.execute()?,
        };
        super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
    }
}

/// Prepared Solarize strategy routed to one exact mode owner.
pub(crate) enum PreparedSolarize<'a> {
    Luma(super::luma8::ops::PreparedSolarize<'a>),
    Palette(super::palette8::ops::PreparedSolarize<'a>),
    Rgb(super::rgb8::ops::PreparedSolarize<'a>),
}

impl PreparedSolarize<'_> {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let mut output = match self {
            Self::Luma(prepared) => prepared.execute()?,
            Self::Palette(prepared) => prepared.execute()?,
            Self::Rgb(prepared) => prepared.execute()?,
        };
        super::helpers::hidden_lanes::initialize_point_output_lanes(&mut output)?;
        Ok(output)
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn prepare_posterize<'a>(
    image: &'a DynamicImage,
    bits: u8,
    ctx: &ExecutionContext,
) -> Result<PreparedPosterize<'a>, PilError> {
    let selection = mode_selection(
        &POSTERIZE_SUPPORT,
        image,
        RequestFacts::Posterize { bits },
        ctx,
    )?;
    match image {
        DynamicImage::ImageLuma8(input) => Ok(PreparedPosterize::Luma(
            super::luma8::ops::PreparedPosterize::new(input, bits, selection),
        )),
        DynamicImage::ImagePalette8(input) => Ok(PreparedPosterize::Palette(
            super::palette8::ops::PreparedPosterize::new(input, bits, selection),
        )),
        DynamicImage::ImageRgb8(input) => Ok(PreparedPosterize::Rgb(
            super::rgb8::ops::PreparedPosterize::new(input, bits, selection),
        )),
        _ => Err(PilError::InternalError(
            "native-byte point support and mode dispatch disagree".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn prepare_solarize<'a>(
    image: &'a DynamicImage,
    threshold: u8,
    ctx: &ExecutionContext,
) -> Result<PreparedSolarize<'a>, PilError> {
    let selection = mode_selection(&SOLARIZE_SUPPORT, image, RequestFacts::Unspecified, ctx)?;
    match image {
        DynamicImage::ImageLuma8(input) => Ok(PreparedSolarize::Luma(
            super::luma8::ops::PreparedSolarize::new(input, threshold, selection),
        )),
        DynamicImage::ImagePalette8(input) => Ok(PreparedSolarize::Palette(
            super::palette8::ops::PreparedSolarize::new(input, threshold, selection),
        )),
        DynamicImage::ImageRgb8(input) => Ok(PreparedSolarize::Rgb(
            super::rgb8::ops::PreparedSolarize::new(input, threshold, selection),
        )),
        _ => Err(PilError::InternalError(
            "native-byte point support and mode dispatch disagree".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    ctx: &ExecutionContext,
) -> Result<PreparedInvert<'a>, PilError> {
    prepare_mode(image, ctx)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
