//! Draw support, prepared CPU selection, and mode-owner routing.
//!
//! Geometry stays in shared helpers; each supported mode and architecture owns
//! its typed Default or vector strategy.
//!
//! Direct Bitmap now selects the exact raster-mode Default strategy through
//! the injected CPU/Architecture context; compatible accelerated requests
//! still delegate to Paste's prepared strategy. `DrawRequestKind` keeps Bitmap
//! and Text requests ineligible for geometry span kernels. Bitmap mask modes
//! and drawing-context relations are validated by the same support declaration
//! used for CPU selection, and its selected raster mode drives the Bitmap
//! executor. Direct Text resolves the same declaration with its raster/context
//! modes, indexed palette presence, mask representation, ink provenance/value, and
//! embedded-color facts, then executes the selected raster owner.
//! Mode-owned Bitmap/Text and Default/SIMD geometry writers preserve omitted
//! physical lanes using the same mode-specific sidecar. TODO(pillow-rs#3ptb):
//! verify I;16, PA L-mask, RGBa PREBLEND, and public Bitmap fast-route outputs
//! in parity; finish direct Text ink and palette/error-order review.
//! Pillow 12.2's `ImagingFill2` confirms I;16 L masks blend both sample bytes,
//! while 1/RGBA/RGBa masks step through the raw row byte prefix; RGBa reads
//! alpha and applies PREBLEND. Draw support declares the compatible
//! accelerated Bitmap route's Paste child and exact raster/context/mask,
//! backend, and nonempty guards; the wrapper evaluates that declaration. The
//! Source tracing confirms the queued child receipt is Paste; route parity
//! remains TODO. Text masks are dispatched without an expanded intermediate
//! buffer; parity remains pending. TODO(pillow-rs#3ptb): continue passing
//! `StrategySelection` into direct Text/Bitmap mode owners and audit the eager
//! native Bitmap fast route.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::compute::registry::support::{
    DrawRequestKind, DrawTextMaskKind, ImplementationSupport, InputContract, ModeSupport,
    OperationId, OperationInputs, OperationModeRoute, OperationRouteGuard,
    OperationRouteInputs, OperationRoutePath, OperationSupport, OutputModeRule,
    PaletteKnowledge, RequestFacts, StrategyGuard, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use super::{BitmapMask, TextComposeInput, TextComposeResult};

const BITMAP_MASK_MODES: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
];
const TEXT_MASK_KINDS: &[DrawTextMaskKind] = &[
    DrawTextMaskKind::Coverage,
    DrawTextMaskKind::Rgba,
];
const PALETTE_MODES: &[Mode] = &[Mode::ImagePalette8, Mode::ImagePaletteAlpha8];
const TEXT_EMBEDDED_COLOR_MODES: &[Mode] = &[Mode::ImageRgb8, Mode::ImageRgba8];
const DRAW_INPUTS: &[crate::compute::registry::support::InputContract] = &[
    crate::compute::registry::support::InputContract::DrawInputs {
        bitmap_masks: BITMAP_MASK_MODES,
        text_mask_kinds: TEXT_MASK_KINDS,
        palette_modes: PALETTE_MODES,
        allow_rgba_context_on_rgb: true,
        embedded_color_modes: TEXT_EMBEDDED_COLOR_MODES,
        mode_rejection: "mode mismatch",
        mask_rejection: "bad transparency mask",
        embedded_color_rejection: "Embedded color supported only in RGB and RGBA modes",
    },
];
const NO_ARCHITECTURES: &[ImplementationSupport] = &[];

const fn draw_vector_architectures(mode: Mode) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            StrategyGuard::DrawNativeSpans,
        ),
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::DrawNativeSpans,
        ),
        ImplementationSupport::cpu(
            OperationId::Draw,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::DrawNativeSpans,
        ),
    ]
}

static ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::One);
static LUMA8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageLuma8);
static LUMA_A8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageLumaA8);
static LA_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageLaPremultiplied8);
static PALETTE8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImagePalette8);
static PALETTE_ALPHA8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImagePaletteAlpha8);
static RGB8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageRgb8);
static RGBA8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageRgba8);
static RGB_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageRgbPremultiplied8);
static RGBX8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageRgbx8);
static CMYK8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageCmyk8);
static YCBCR8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageYCbCr8);
static LAB8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageLab8);
static HSV8_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageHsv8);
static I32_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageI32);
static F32_ARCHITECTURES: [ImplementationSupport; 3] =
    draw_vector_architectures(Mode::ImageF32);

const DRAW_COMPOSED_OPERATIONS: &[OperationId] = &[OperationId::Paste];
const DRAW_ACCELERATED_BACKENDS: &[Backend] = &[Backend::Gpu, Backend::Simd];
const DRAW_BITMAP_MASKED_PASTE_ROUTES: &[(Mode, Mode, Mode)] = &[
    (Mode::One, Mode::One, Mode::One),
    (Mode::ImageLuma8, Mode::ImageLuma8, Mode::One),
    (Mode::ImageLuma8, Mode::ImageLuma8, Mode::ImageLuma8),
    (Mode::ImageLumaA8, Mode::ImageLumaA8, Mode::One),
    (Mode::ImagePalette8, Mode::ImagePalette8, Mode::One),
    (Mode::ImageRgb8, Mode::ImageRgb8, Mode::One),
    (Mode::ImageRgb8, Mode::ImageRgb8, Mode::ImageLuma8),
    (Mode::ImageRgb8, Mode::ImageRgba8, Mode::One),
    (Mode::ImageRgb8, Mode::ImageRgba8, Mode::ImageLuma8),
    (Mode::ImageRgba8, Mode::ImageRgba8, Mode::One),
    (Mode::ImageCmyk8, Mode::ImageCmyk8, Mode::One),
    (Mode::ImageCmyk8, Mode::ImageCmyk8, Mode::ImageLuma8),
    (Mode::ImageHsv8, Mode::ImageHsv8, Mode::One),
    (Mode::ImageHsv8, Mode::ImageHsv8, Mode::ImageLuma8),
];
const DRAW_BITMAP_MASKED_PASTE_GUARDS: &[OperationRouteGuard] = &[
    OperationRouteGuard::RequestedBackendIn(DRAW_ACCELERATED_BACKENDS),
    OperationRouteGuard::NonEmptyInput,
    OperationRouteGuard::DrawBitmapRouteIn(DRAW_BITMAP_MASKED_PASTE_ROUTES),
];
const DRAW_BITMAP_MASKED_PASTE_PATHS: &[OperationRoutePath] =
    &[OperationRoutePath::ConditionalPipelineChild {
        operation: OperationId::Paste,
        guards: DRAW_BITMAP_MASKED_PASTE_GUARDS,
    }];

fn classify_route_paths(mode: Mode) -> &'static [OperationRoutePath] {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImagePalette8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageCmyk8
        | Mode::ImageHsv8 => DRAW_BITMAP_MASKED_PASTE_PATHS,
        _ => &[],
    }
}

fn classify_route(mode: Mode) -> OperationModeRoute {
    OperationModeRoute::Prepared {
        support: classify_mode(mode),
        intermediate_mode: None,
        paths: classify_route_paths(mode),
    }
}

pub(crate) static SUPPORT: OperationSupport = OperationSupport::routed(
    OperationId::Draw,
    DRAW_INPUTS,
    classify_route,
    DRAW_COMPOSED_OPERATIONS,
);

/// Validates the bitmap mode using the mask identities declared by Draw support.
pub(crate) fn validate_bitmap_mask_mode(mode: Mode) -> Result<(), PilError> {
    let contract = SUPPORT.inputs.iter().find_map(|contract| {
        let InputContract::DrawInputs {
            bitmap_masks,
            mask_rejection,
            ..
        } = contract
        else {
            return None;
        };
        Some((*bitmap_masks, *mask_rejection))
    });
    let Some((bitmap_masks, mask_rejection)) = contract else {
        return Err(PilError::InternalError(
            "Draw support omitted its bitmap mask contract".into(),
        ));
    };
    if bitmap_masks.contains(&mode) {
        Ok(())
    } else {
        Err(PilError::ValueError(mask_rejection.to_owned()))
    }
}

pub(crate) struct PreparedDraw<'a> {
    image: &'a DynamicImage,
    op: &'a PipelineOp,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedDraw<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_batch(
            self.image,
            std::slice::from_ref(self.op),
            self.mode,
            self.selection,
        )
    }
}

pub(crate) fn mode_support(mode: Mode) -> ModeSupport {
    let architectures = match mode {
        Mode::One => &ONE_ARCHITECTURES,
        Mode::ImageLuma8 => &LUMA8_ARCHITECTURES,
        Mode::ImageLumaA8 => &LUMA_A8_ARCHITECTURES,
        Mode::ImageLaPremultiplied8 => &LA_PREMULTIPLIED8_ARCHITECTURES,
        Mode::ImagePalette8 => &PALETTE8_ARCHITECTURES,
        Mode::ImagePaletteAlpha8 => &PALETTE_ALPHA8_ARCHITECTURES,
        Mode::ImageRgb8 => &RGB8_ARCHITECTURES,
        Mode::ImageRgba8 => &RGBA8_ARCHITECTURES,
        Mode::ImageRgbPremultiplied8 => &RGB_PREMULTIPLIED8_ARCHITECTURES,
        Mode::ImageRgbx8 => &RGBX8_ARCHITECTURES,
        Mode::ImageCmyk8 => &CMYK8_ARCHITECTURES,
        Mode::ImageYCbCr8 => &YCBCR8_ARCHITECTURES,
        Mode::ImageLab8 => &LAB8_ARCHITECTURES,
        Mode::ImageHsv8 => &HSV8_ARCHITECTURES,
        Mode::ImageI32 => &I32_ARCHITECTURES,
        Mode::ImageF32 => &F32_ARCHITECTURES,
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => NO_ARCHITECTURES,
    };
    ModeSupport::cpu(
        OperationId::Draw,
        mode,
        OutputModeRule::Preserve,
        DRAW_INPUTS,
        architectures,
    )
}

/// Selects the exact raster-mode strategy for direct Bitmap execution.
///
/// Direct Bitmap loops do not use the native span kernels selected by Draw's
/// geometry batch, so a non-Default architecture request resolves to this
/// mode's Default strategy.
pub(crate) fn select_bitmap_strategy(
    support: ModeSupport,
    context: &ExecutionContext,
    raster_mode: Mode,
    context_mode: Mode,
    bitmap_mask_mode: Mode,
    palette: PaletteKnowledge,
) -> Result<StrategySelection, PilError> {
    select_direct_strategy(
        support,
        context,
        palette,
        RequestFacts::Draw {
            kind: DrawRequestKind::Bitmap,
            all_geometry_spans_eligible: false,
            raster_mode: Some(raster_mode),
            context_mode: Some(context_mode),
            bitmap_mask_mode: Some(bitmap_mask_mode),
            embedded_color: None,
            text_mask_kind: None,
            text_fill_is_integer: None,
            text_ink: None,
        },
    )
}

/// Selects the exact raster-mode strategy for direct Text composition.
pub(crate) fn select_text_strategy(
    support: ModeSupport,
    context: &ExecutionContext,
    raster_mode: Mode,
    context_mode: Mode,
    embedded_color: bool,
    text_mask_kind: DrawTextMaskKind,
    text_fill_is_integer: bool,
    text_ink: Option<i64>,
    palette: PaletteKnowledge,
) -> Result<StrategySelection, PilError> {
    select_direct_strategy(
        support,
        context,
        palette,
        RequestFacts::Draw {
            kind: DrawRequestKind::Text,
            all_geometry_spans_eligible: false,
            raster_mode: Some(raster_mode),
            context_mode: Some(context_mode),
            bitmap_mask_mode: None,
            embedded_color: Some(embedded_color),
            text_mask_kind: Some(text_mask_kind),
            text_fill_is_integer: Some(text_fill_is_integer),
            text_ink,
        },
    )
}

/// Executes Text through the exact mode selected by the shared Draw contract.
pub(crate) fn apply_text(
    selection: StrategySelection,
    input: TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    let mode = selection.implementation.mode;
    validate_direct_selection(selection, mode)?;
    match input.mask_kind {
        DrawTextMaskKind::Coverage => apply_text_for_mode::<false>(mode, input),
        DrawTextMaskKind::Rgba => apply_text_for_mode::<true>(mode, input),
    }
}

fn apply_text_for_mode<const RGBA_MASK: bool>(
    mode: Mode,
    input: TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    match mode {
        Mode::One => super::one::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageLuma8 => super::luma8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageLumaA8 => super::luma_a8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::apply_text::<RGBA_MASK>(input)
        }
        Mode::ImagePalette8 => super::palette8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageRgb8 => super::rgb8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageRgba8 => super::rgba8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::apply_text::<RGBA_MASK>(input)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageCmyk8 => super::cmyk8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageLab8 => super::lab8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageHsv8 => super::hsv8::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageI32 => super::i32::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageF32 => super::f32::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageI16 => super::i16::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageI16L => super::i16l::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageI16B => super::i16b::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageI16N => super::i16n::ops::apply_text::<RGBA_MASK>(input),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::ValueError("unsupported image mode".into())),
    }
}

/// Applies the prepared bitmap through the selected exact-mode Default owner.
pub(crate) fn apply_bitmap(
    selection: StrategySelection,
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let mode = selection.implementation.mode;
    validate_direct_selection(selection, mode)?;
    match mode {
        Mode::One => super::one::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageLuma8 => super::luma8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageLumaA8 => super::luma_a8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImagePalette8 => super::palette8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageRgb8 => super::rgb8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageRgba8 => super::rgba8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageRgbx8 => super::rgbx8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageCmyk8 => super::cmyk8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageLab8 => super::lab8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageHsv8 => super::hsv8::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageI32 => super::i32::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageF32 => super::f32::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageI16 => super::i16::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageI16L => super::i16l::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageI16B => super::i16b::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageI16N => super::i16n::ops::apply_bitmap(
            image, x, y, mask_width, mask_height, mask, color,
        ),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => {
            // NO_PILLOW_SUPPORT: these internal modes have no byte-mask drawing contract.
            Err(PilError::ValueError("unsupported image mode".into()))
        }
    }
}

fn select_direct_strategy(
    support: ModeSupport,
    context: &ExecutionContext,
    palette: PaletteKnowledge,
    request: RequestFacts,
) -> Result<StrategySelection, PilError> {
    let input = StrategyInputs {
        width: 0,
        height: 0,
        byte_len: None,
        palette,
        request,
    };
    match support.assess(context, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "direct Draw preparation received unresolved input facts".into(),
        )),
    }
}

/// Confirms a direct Draw executor received its exact mode and Default strategy.
pub(crate) fn validate_direct_selection(
    selection: StrategySelection,
    mode: Mode,
) -> Result<(), PilError> {
    if selection.implementation.operation != OperationId::Draw
        || selection.implementation.mode != mode
        || selection.implementation.backend != Backend::Cpu
        || selection.implementation.architecture != Architecture::Default
    {
        return Err(PilError::InternalError(
            "prepared direct Draw selection does not match its raster mode".into(),
        ));
    }
    Ok(())
}

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::support(),
        Mode::ImageLuma8 => super::luma8::ops::support(),
        Mode::ImageLumaA8 => super::luma_a8::ops::support(),
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::support(),
        Mode::ImagePalette8 => super::palette8::ops::support(),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::support(),
        Mode::ImageRgb8 => super::rgb8::ops::support(),
        Mode::ImageRgba8 => super::rgba8::ops::support(),
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::support(),
        Mode::ImageRgbx8 => super::rgbx8::ops::support(),
        Mode::ImageCmyk8 => super::cmyk8::ops::support(),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::support(),
        Mode::ImageLab8 => super::lab8::ops::support(),
        Mode::ImageHsv8 => super::hsv8::ops::support(),
        Mode::ImageI32 => super::i32::ops::support(),
        Mode::ImageF32 => super::f32::ops::support(),
        Mode::ImageI16 => super::i16::ops::support(),
        Mode::ImageI16L => super::i16l::ops::support(),
        Mode::ImageI16B => super::i16b::ops::support(),
        Mode::ImageI16N => super::i16n::ops::support(),
        Mode::ImageLumaA16 => super::luma_a16::ops::support(),
        Mode::ImageRgb16 => super::rgb16::ops::support(),
        Mode::ImageRgba16 => super::rgba16::ops::support(),
        Mode::ImageRgb32F => super::rgb32f::ops::support(),
        Mode::ImageRgba32F => super::rgba32f::ops::support(),
    }
}

fn prepare_mode(
    image: &DynamicImage,
    ops: &[PipelineOp],
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let mode = image.mode();
    let request = if context.architecture() == Architecture::Default
        || !context.supports_architecture(context.architecture())
    {
        RequestFacts::Unspecified
    } else {
        RequestFacts::Draw {
            kind: DrawRequestKind::GeometryBatch,
            all_geometry_spans_eligible: super::helpers::simd::is_eligible(image, ops, mode),
            raster_mode: Some(mode),
            context_mode: Some(mode),
            bitmap_mask_mode: None,
            embedded_color: None,
            text_mask_kind: None,
            text_fill_is_integer: None,
            text_ink: None,
        }
    };
    let input = StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
        palette: PaletteKnowledge::Absent,
        request,
    };
    SUPPORT.for_mode(mode).select(context, input)
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<PreparedDraw<'a>, PilError> {
    if !is_draw_op(op) {
        return Err(PilError::ValueError("expected Draw operation".into()));
    }
    super::helpers::validate_draw_ops(std::slice::from_ref(op))?;
    let mode = input.mode();
    let selection = prepare_mode(input.image, std::slice::from_ref(op), context)?;
    Ok(PreparedDraw {
        image: input.image,
        op,
        mode,
        selection,
    })
}

pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    prepare(input, op, context).map(crate::compute::registry::PreparedOperation::from_draw)
}

pub(crate) fn execute_batch(
    image: &DynamicImage,
    ops: &[PipelineOp],
    mode: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let implementation = selection.implementation;
    let declared = match SUPPORT.for_mode(mode) {
        ModeSupport::Supported {
            default,
            architectures,
            ..
        } => {
            default.id == implementation
                || architectures
                    .iter()
                    .any(|architecture| architecture.id == implementation)
        }
        ModeSupport::Rejected { .. } => false,
    };
    if image.mode() != mode
        || implementation.operation != OperationId::Draw
        || implementation.mode != mode
        || implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
        || !declared
    {
        return Err(PilError::InternalError(
            "prepared Draw selection does not match its mode-bearing raster".into(),
        ));
    }
    super::helpers::validate_draw_ops(ops)?;
    execute_mode(image, ops, mode, &selection)
}

pub(crate) fn prepare_batch(
    image: &DynamicImage,
    ops: &[PipelineOp],
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    super::helpers::validate_draw_ops(ops)?;
    prepare_mode(image, ops, context)
}

fn execute_mode(
    image: &DynamicImage,
    ops: &[PipelineOp],
    mode: Mode,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One => super::one::ops::execute(image, ops, selection),
        Mode::ImageLuma8 => {
            super::luma8::ops::execute(image, ops, selection)
        }
        Mode::ImageLumaA8 => {
            super::luma_a8::ops::execute(image, ops, selection)
        }
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::execute(image, ops, selection)
        }
        Mode::ImagePalette8 => {
            super::palette8::ops::execute(image, ops, selection)
        }
        Mode::ImagePaletteAlpha8 => {
            super::palette_alpha8::ops::execute(image, ops, selection)
        }
        Mode::ImageRgb8 => {
            super::rgb8::ops::execute(image, ops, selection)
        }
        Mode::ImageRgba8 => {
            super::rgba8::ops::execute(image, ops, selection)
        }
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::execute(image, ops, selection)
        }
        Mode::ImageRgbx8 => {
            super::rgbx8::ops::execute(image, ops, selection)
        }
        Mode::ImageCmyk8 => {
            super::cmyk8::ops::execute(image, ops, selection)
        }
        Mode::ImageYCbCr8 => {
            super::ycbcr8::ops::execute(image, ops, selection)
        }
        Mode::ImageLab8 => {
            super::lab8::ops::execute(image, ops, selection)
        }
        Mode::ImageHsv8 => {
            super::hsv8::ops::execute(image, ops, selection)
        }
        Mode::ImageI32 => {
            super::i32::ops::execute(image, ops, selection)
        }
        Mode::ImageF32 => {
            super::f32::ops::execute(image, ops, selection)
        }
        Mode::ImageI16 => super::i16::ops::execute(image, ops, selection),
        Mode::ImageI16L => super::i16l::ops::execute(image, ops, selection),
        Mode::ImageI16B => super::i16b::ops::execute(image, ops, selection),
        Mode::ImageI16N => super::i16n::ops::execute(image, ops, selection),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::ValueError("unsupported image mode".into())),
    }
}


pub(crate) fn is_draw_op(op: &PipelineOp) -> bool {
    super::helpers::is_draw_op(op)
}
