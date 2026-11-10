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
        SupportAssessment, SupportScope,
    },
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

pub(crate) use super::autocontrast::PreparedAutocontrast;
pub(crate) use super::equalize::PreparedEqualize;
pub(crate) use super::expand::PreparedExpand;
use super::expand::{prepare as prepare_expand, prepare_with_native_fill};
pub(crate) use super::luma8::ops::PreparedColorize;

macro_rules! declare_mode_support {
    ($name:ident, $operation:ident, $module:ident) => {
        pub(crate) static $name: OperationSupport = OperationSupport::new(
            OperationId::$operation,
            super::$module::INPUTS,
            super::$module::classify_mode,
        );
    };
}

// Family-level support declarations are owned by the ImageOps entrypoint.
declare_mode_support!(AUTOCONTRAST_SUPPORT, Autocontrast, autocontrast);
declare_mode_support!(EQUALIZE_SUPPORT, Equalize, equalize);

/// Complete exact-mode support for native ImageOps.expand.
pub(crate) static EXPAND_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Expand, &[], super::expand::classify_mode);

/// Exact-mode support for nearest-edge expansion on the callback handle.
pub(crate) static IMAGING_CORE_EXPAND_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ImagingCoreExpand,
    &[],
    super::expand_edges::classify_mode,
);

declare_mode_support!(QUANTIZE_SUPPORT, Quantize, quantize);

pub(crate) fn prepare_autocontrast_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    ctx: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    let PipelineOp::Autocontrast { options, mask } = op else {
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    };
    super::autocontrast::prepare(input.image, options, mask.as_ref(), ctx)
        .map(crate::compute::registry::PreparedOperation::from_autocontrast)
}

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
    super::equalize::prepare(input, mask, ctx)
        .map(crate::compute::registry::PreparedOperation::from_equalize)
}

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

fn rejected(mode: Mode, reason: &'static str) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::InternalError,
    }
}

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

const IMAGEOPS_INVERT_CHILDREN: &[OperationId] = &[OperationId::Eval];

fn imageops_invert_rejection(mode: Mode) -> Option<(&'static str, RejectionKind)> {
    imageops_lut_wrapper_rejection(mode).or_else(|| match mode {
        Mode::ImageLumaA8 => Some(("not supported for mode LA", RejectionKind::OsError)),
        Mode::ImagePaletteAlpha8 => Some(("not supported for mode PA", RejectionKind::OsError)),
        Mode::ImageRgba8 => Some(("not supported for mode RGBA", RejectionKind::OsError)),
        Mode::ImageCmyk8 => Some(("not supported for mode CMYK", RejectionKind::OsError)),
        Mode::ImageI32 => Some(("not supported for mode I", RejectionKind::OsError)),
        Mode::ImageF32 => Some(("not supported for mode F", RejectionKind::OsError)),
        Mode::ImageI16 => Some(("not supported for mode I;16", RejectionKind::OsError)),
        Mode::ImageI16L => Some(("not supported for mode I;16L", RejectionKind::OsError)),
        Mode::ImageI16B => Some(("not supported for mode I;16B", RejectionKind::OsError)),
        Mode::ImageI16N => Some(("not supported for mode I;16N", RejectionKind::OsError)),
        _ => None,
    })
}

fn classify_imageops_invert_route(mode: Mode) -> OperationModeRoute {
    let child_support = OperationId::Eval.support().for_mode(mode);
    let wrapper_rejection = imageops_invert_rejection(mode);
    let scope = match (wrapper_rejection, child_support) {
        (Some(_), _) if mode.is_pillow_mode() => SupportScope::Pillow,
        (_, ModeSupport::Supported { scope, .. }) => scope,
        (_, ModeSupport::Rejected { .. }) if mode.is_pillow_mode() => SupportScope::Pillow,
        _ => SupportScope::CoreOnly {
            reason: "this internal raster identity has no Pillow ImageOps route",
        },
    };
    let (support, operations, outcome) = match wrapper_rejection {
        Some((reason, kind)) => (
            ModeSupport::Rejected { mode, reason, kind },
            &[][..],
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::WrapperGuard,
                reason,
                kind,
            },
        ),
        None => match child_support {
            ModeSupport::Supported { .. } => (
                child_support,
                IMAGEOPS_INVERT_CHILDREN,
                OperationRouteOutcome::Produces,
            ),
            ModeSupport::Rejected { reason, kind, .. } => (
                child_support,
                IMAGEOPS_INVERT_CHILDREN,
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::ChildAdmission,
                    reason,
                    kind,
                },
            ),
        },
    };

    OperationModeRoute::ComposedFallback {
        mode,
        intermediate_mode: None,
        scope,
        operations,
        output: OutputModeRule::Preserve,
        outcome,
        prepared_support: support,
    }
}

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

const IMAGEOPS_LUT_CHILDREN: &[OperationId] = &[OperationId::Eval];

// TODO(pillow-rs#3ptb): verify wrapper mode/fault order, custom mode and point
// callbacks, unsupported-mode formatting faults, and selected/fallback
// receipts for ImageOps Invert, Posterize, and Solarize. Their Rust-native
// operation contracts remain separate.
fn imageops_lut_mode_support(mode: Mode) -> Option<ModeSupport> {
    if !matches!(mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return None;
    }

    let support = OperationId::Eval.support().for_mode(mode);
    let scope = match support {
        ModeSupport::Supported { scope, .. } => scope,
        ModeSupport::Rejected { .. } => return Some(support),
    };

    Some(match support {
        ModeSupport::Supported {
            default,
            architectures,
            output,
            inputs,
            value_result,
            ..
        } => ModeSupport::Supported {
            scope,
            default,
            architectures,
            output,
            inputs,
            value_result,
        },
        rejected => rejected,
    })
}

fn imageops_lut_wrapper_rejection(mode: Mode) -> Option<(&'static str, RejectionKind)> {
    match mode {
        // Pillow's shared _lut wrapper rejects P; direct Rust operations
        // retain their separately declared mode strategies.
        Mode::ImagePalette8 => Some((
            "mode P support coming soon",
            RejectionKind::NotImplementedError,
        )),
        _ => None,
    }
}

fn classify_imageops_lut_route(mode: Mode) -> OperationModeRoute {
    let pillow_mode = mode.is_pillow_mode();
    let support_scope = if pillow_mode {
        SupportScope::Pillow
    } else {
        SupportScope::CoreOnly {
            reason: "this internal raster identity has no Pillow ImageOps route",
        }
    };
    let (support, operations, scope, outcome) = match (
        imageops_lut_wrapper_rejection(mode),
        imageops_lut_mode_support(mode),
    ) {
        (Some((reason, kind)), _) => (
            ModeSupport::Rejected { mode, reason, kind },
            &[][..],
            support_scope,
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::WrapperGuard,
                reason,
                kind,
            },
        ),
        (None, Some(support @ ModeSupport::Supported { scope, .. })) => (
            support,
            IMAGEOPS_LUT_CHILDREN,
            scope,
            OperationRouteOutcome::Produces,
        ),
        (None, Some(support @ ModeSupport::Rejected { reason, kind, .. })) => (
            support,
            IMAGEOPS_LUT_CHILDREN,
            support_scope,
            OperationRouteOutcome::Rejects {
                stage: OperationRouteStage::ChildAdmission,
                reason,
                kind,
            },
        ),
        (None, None) => {
            let reason = super::helpers::strategy::unsupported_mode_reason(mode);
            let kind = RejectionKind::OsError;
            (
                ModeSupport::Rejected { mode, reason, kind },
                &[][..],
                support_scope,
                OperationRouteOutcome::Rejects {
                    stage: OperationRouteStage::WrapperGuard,
                    reason,
                    kind,
                },
            )
        }
    };

    OperationModeRoute::ComposedFallback {
        mode,
        intermediate_mode: None,
        scope,
        operations,
        output: OutputModeRule::Preserve,
        outcome,
        prepared_support: support,
    }
}

fn classify_imageops_posterize_route(mode: Mode) -> OperationModeRoute {
    classify_imageops_lut_route(mode)
}

fn classify_imageops_solarize_route(mode: Mode) -> OperationModeRoute {
    classify_imageops_lut_route(mode)
}

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

fn classify_native_byte_point_mode(
    mode: Mode,
    luma_support: ModeSupport,
    palette_support: ModeSupport,
    rgb_support: ModeSupport,
) -> ModeSupport {
    match mode {
        Mode::ImageLuma8 => luma_support,
        Mode::ImageRgb8 => rgb_support,
        Mode::ImagePalette8 => palette_support,
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

fn rejected_native_byte_point_mode(mode: Mode) -> ModeSupport {
    let reason = match mode {
        Mode::One => "not supported for mode 1",
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImagePalette8 => "not supported for this mode",
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
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => super::helpers::strategy::unsupported_mode_reason(mode),
    };
    ModeSupport::Rejected {
        mode,
        reason,
        kind: RejectionKind::OsError,
    }
}

fn classify_posterize_mode(mode: Mode) -> ModeSupport {
    classify_native_byte_point_mode(
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

fn classify_solarize_mode(mode: Mode) -> ModeSupport {
    classify_native_byte_point_mode(
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
        fn classify_colorize_mode(mode: Mode) -> ModeSupport {
            match mode {
                Mode::ImageLuma8 => super::luma8::ops::COLORIZE_SUPPORT,
                $(Mode::$mode => rejected_colorize_mode(mode),)+
            }
        }
    };
}

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
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

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
    pub(crate) fn selection(&self) -> &StrategySelection {
        match self {
            Self::Luma(prepared) => prepared.selection(),
            Self::Palette(prepared) => prepared.selection(),
            Self::Rgb(prepared) => prepared.selection(),
        }
    }

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
