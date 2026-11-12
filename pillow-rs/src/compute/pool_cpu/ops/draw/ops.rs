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
//! physical lanes using the same mode-specific sidecar. Focused CPU parity
//! covers every supported Bitmap mask against every public byte mode.
//! TODO(pillow-rs#3ptb): verify direct Bitmap/Text selected-route receipts and
//! backend fallback selection across operation × mode × architecture.
//! TODO(pillow-rs#3ptb): add direct Bitmap parity at `i32::MIN`/`i32::MAX`
//! origins for every supported byte mode after the CPU source migration closes.
//! The five internal high-depth modes report an explicit ValueError rejection
//! because no byte-mask Bitmap/Text strategy is implemented for them.
//! Pillow 12.2's `ImagingFill2` confirms I;16 L masks blend both sample bytes,
//! while 1/RGBA/RGBa masks step through the raw row byte prefix; RGBa reads
//! alpha and applies PREBLEND. Draw support declares the compatible
//! accelerated Bitmap route's Paste child and exact raster/context/mask,
//! backend, and nonempty guards; the wrapper evaluates that declaration. The
//! Source tracing confirms the queued child receipt is Paste; route parity
//! remains TODO. Text masks are dispatched without an expanded intermediate
//! buffer; the complete mode and route receipt matrix remains TODO. The Python
//! facade and direct PyO3 Bitmap entry load the mask before fill
//! parsing/palette resolution. Bitmap ordering
//! is source-reviewed: both resolve fill before coordinates, then core validates
//! the mask and selects its declared strategy. The RGB and CMYK eager paths run
//! only after that selection. Direct Text resolves fill and conditional stroke
//! ink in the Pillow drawing context before line layout. Focused CPU cases
//! cover selected native modes, F-mode packed ink, and callback/error behavior;
//! complete output, fault, and route receipts remain parity work.

use super::{BitmapMask, TextComposeInput, TextComposeResult};
use crate::compute::registry::support::{
    DrawRequestKind, DrawTextMaskKind, InputContract, ModeSupport, OperationId, OperationInputs,
    OperationModeRoute, OperationRouteGuard, OperationRoutePath, OperationSupport,
    OutputModeRule, PaletteKnowledge, RequestFacts, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::draw::{FloodFillAccess, FloodFillError, PreparedFloodFill};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

const BITMAP_MASK_MODES: &[Mode] = &[
    Mode::One,
    Mode::ImageLuma8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
];
const TEXT_MASK_KINDS: &[DrawTextMaskKind] = &[DrawTextMaskKind::Coverage, DrawTextMaskKind::Rgba];
const PALETTE_MODES: &[Mode] = &[Mode::ImagePalette8, Mode::ImagePaletteAlpha8];
const TEXT_EMBEDDED_COLOR_MODES: &[Mode] = &[Mode::ImageRgb8, Mode::ImageRgba8];
pub(in crate::compute::pool_cpu::ops::draw) const DRAW_INPUTS: &[InputContract] = &[
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

const FLOODFILL_INPUTS: &[InputContract] = &[InputContract::FloodFillInputs];

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_floodfill_mode(mode: Mode) -> ModeSupport {
    match mode {
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
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => pillow_floodfill_strategy(mode),
        // NO_PILLOW_SUPPORT: FloodFill accepts these internal high-depth
        // identities through the native Rust callback contract only.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::FloodFill,
            mode,
            OutputModeRule::NoRasterResult,
            FLOODFILL_INPUTS,
            &[],
            "internal high-depth raster identity has no Pillow floodfill contract",
        ),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn pillow_floodfill_strategy(mode: Mode) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::FloodFill,
        mode,
        OutputModeRule::NoRasterResult,
        FLOODFILL_INPUTS,
        &[],
    )
}

pub(crate) static FLOODFILL_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::FloodFill,
    FLOODFILL_INPUTS,
    classify_floodfill_mode,
);

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_floodfill(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<PreparedFloodFill, PilError> {
    let selection = FLOODFILL_SUPPORT.for_mode(mode).select_serial(context)?;
    Ok(PreparedFloodFill::new(mode, selection))
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute_floodfill<A: FloodFillAccess>(
    prepared: &PreparedFloodFill,
    access: &mut A,
    seed: A::Point,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    threshold: &A::Threshold,
) -> Result<(), FloodFillError<A::Error>> {
    let implementation = prepared.selection().implementation;
    if implementation.operation != OperationId::FloodFill
        || implementation.mode != prepared.mode()
        || implementation.backend != Backend::Cpu
        || implementation.architecture != Architecture::Default
        || prepared.selection().requested_backend != Backend::Cpu
    {
        return Err(FloodFillError::Strategy(PilError::InternalError(
            "FloodFill received a mismatched CPU Default strategy selection".into(),
        )));
    }
    super::helpers::floodfill::run(access, seed, value, border, threshold)
        .map_err(FloodFillError::Callback)
}

/// Validates the bitmap mode using the mask identities declared by Draw support.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        execute_batch(
            self.image,
            std::slice::from_ref(self.op),
            self.mode,
            self.selection,
        )
    }
}

/// Selects the exact raster-mode strategy for direct Bitmap execution.
///
/// Direct Bitmap loops do not use the native span kernels selected by Draw's
/// geometry batch, so a non-Default architecture request resolves to this
/// mode's Default strategy.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
            text_ink: None,
        },
    )
}

/// Selects the exact raster-mode strategy for direct Text composition.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn select_text_strategy(
    support: ModeSupport,
    context: &ExecutionContext,
    raster_mode: Mode,
    context_mode: Mode,
    embedded_color: bool,
    text_mask_kind: DrawTextMaskKind,
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
            text_ink,
        },
    )
}

/// Executes Text through the exact mode selected by the shared Draw contract.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn apply_text(
    selection: StrategySelection,
    input: TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    let mode = selection.implementation.mode;
    validate_direct_selection(selection, mode)?;
    match input.mask_kind {
        DrawTextMaskKind::Coverage => apply_text_for_mode::<false>(mode, input, &selection),
        DrawTextMaskKind::Rgba => apply_text_for_mode::<true>(mode, input, &selection),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn apply_text_for_mode<const RGBA_MASK: bool>(
    mode: Mode,
    input: TextComposeInput<'_>,
    selection: &StrategySelection,
) -> Result<TextComposeResult, PilError> {
    match mode {
        Mode::One => super::one::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageLuma8 => super::luma8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageLumaA8 => super::luma_a8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::apply_text::<RGBA_MASK>(selection, input)
        }
        Mode::ImagePalette8 => super::palette8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImagePaletteAlpha8 => {
            super::palette_alpha8::ops::apply_text::<RGBA_MASK>(selection, input)
        }
        Mode::ImageRgb8 => super::rgb8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageRgba8 => super::rgba8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::apply_text::<RGBA_MASK>(selection, input)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageCmyk8 => super::cmyk8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageLab8 => super::lab8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageHsv8 => super::hsv8::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageI32 => super::i32::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageF32 => super::f32::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageI16 => super::i16::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageI16L => super::i16l::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageI16B => super::i16b::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageI16N => super::i16n::ops::apply_text::<RGBA_MASK>(selection, input),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::ValueError("unsupported image mode".into())),
    }
}

/// Applies the prepared bitmap through the selected exact-mode Default owner.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
    macro_rules! apply_mode {
        ($implementation:path) => {
            $implementation(
                &selection,
                image,
                x,
                y,
                mask_width,
                mask_height,
                mask,
                color,
            )
        };
    }

    match mode {
        Mode::One => apply_mode!(super::one::ops::apply_bitmap),
        Mode::ImageLuma8 => apply_mode!(super::luma8::ops::apply_bitmap),
        Mode::ImageLumaA8 => apply_mode!(super::luma_a8::ops::apply_bitmap),
        Mode::ImageLaPremultiplied8 => apply_mode!(super::la_premultiplied8::ops::apply_bitmap),
        Mode::ImagePalette8 => apply_mode!(super::palette8::ops::apply_bitmap),
        Mode::ImagePaletteAlpha8 => apply_mode!(super::palette_alpha8::ops::apply_bitmap),
        Mode::ImageRgb8 => apply_mode!(super::rgb8::ops::apply_bitmap),
        Mode::ImageRgba8 => apply_mode!(super::rgba8::ops::apply_bitmap),
        Mode::ImageRgbPremultiplied8 => apply_mode!(super::rgb_premultiplied8::ops::apply_bitmap),
        Mode::ImageRgbx8 => apply_mode!(super::rgbx8::ops::apply_bitmap),
        Mode::ImageCmyk8 => apply_mode!(super::cmyk8::ops::apply_bitmap),
        Mode::ImageYCbCr8 => apply_mode!(super::ycbcr8::ops::apply_bitmap),
        Mode::ImageLab8 => apply_mode!(super::lab8::ops::apply_bitmap),
        Mode::ImageHsv8 => apply_mode!(super::hsv8::ops::apply_bitmap),
        Mode::ImageI32 => apply_mode!(super::i32::ops::apply_bitmap),
        Mode::ImageF32 => apply_mode!(super::f32::ops::apply_bitmap),
        Mode::ImageI16 => apply_mode!(super::i16::ops::apply_bitmap),
        Mode::ImageI16L => apply_mode!(super::i16l::ops::apply_bitmap),
        Mode::ImageI16B => apply_mode!(super::i16b::ops::apply_bitmap),
        Mode::ImageI16N => apply_mode!(super::i16n::ops::apply_bitmap),
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_pipeline<'a>(
    input: OperationInputs<'a>,
    op: &'a PipelineOp,
    context: &ExecutionContext,
) -> Result<crate::compute::registry::PreparedOperation<'a>, PilError> {
    prepare(input, op, context).map(crate::compute::registry::PreparedOperation::from_draw)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_batch(
    image: &DynamicImage,
    ops: &[PipelineOp],
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    super::helpers::validate_draw_ops(ops)?;
    prepare_mode(image, ops, context)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn execute_mode(
    image: &DynamicImage,
    ops: &[PipelineOp],
    mode: Mode,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One => super::one::ops::execute(image, ops, selection),
        Mode::ImageLuma8 => super::luma8::ops::execute(image, ops, selection),
        Mode::ImageLumaA8 => super::luma_a8::ops::execute(image, ops, selection),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::execute(image, ops, selection)
        }
        Mode::ImagePalette8 => super::palette8::ops::execute(image, ops, selection),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::execute(image, ops, selection),
        Mode::ImageRgb8 => super::rgb8::ops::execute(image, ops, selection),
        Mode::ImageRgba8 => super::rgba8::ops::execute(image, ops, selection),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::execute(image, ops, selection)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::execute(image, ops, selection),
        Mode::ImageCmyk8 => super::cmyk8::ops::execute(image, ops, selection),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::execute(image, ops, selection),
        Mode::ImageLab8 => super::lab8::ops::execute(image, ops, selection),
        Mode::ImageHsv8 => super::hsv8::ops::execute(image, ops, selection),
        Mode::ImageI32 => super::i32::ops::execute(image, ops, selection),
        Mode::ImageF32 => super::f32::ops::execute(image, ops, selection),
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn is_draw_op(op: &PipelineOp) -> bool {
    super::helpers::is_draw_op(op)
}
