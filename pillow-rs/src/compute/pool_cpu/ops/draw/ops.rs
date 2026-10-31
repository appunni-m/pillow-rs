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
//! modes, P palette presence, mask representation, ink provenance/value, and
//! embedded-color facts, then executes the selected raster owner.
//! TODO(pillow-rs#3ptb): review all Draw contracts against preserved sources
//! before completion.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::compute::registry::support::{
    DrawRequestKind, DrawTextMaskKind, ImplementationSupport, ModeSupport, OperationId,
    OperationInputs, OperationSupport, OutputModeRule, PaletteKnowledge, RequestFacts,
    StrategyGuard, StrategyInputs, StrategySelection, SupportAssessment,
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
const PALETTE_MODES: &[Mode] = &[Mode::ImagePalette8];
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

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Draw, DRAW_INPUTS, classify_mode);

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
    mut input: TextComposeInput<'_>,
) -> Result<TextComposeResult, PilError> {
    let mode = selection.implementation.mode;
    validate_direct_selection(selection, mode)?;
    if input.mask_kind == DrawTextMaskKind::Coverage && mode != Mode::ImageRgb8 {
        let expanded = super::helpers::text_mask::expand_coverage_to_rgba(
            input.pixels.as_ref(),
            (input.fill.0, input.fill.1, input.fill.2),
        );
        input.pixels = std::borrow::Cow::Owned(expanded);
    }
    match mode {
        Mode::One => super::one::ops::apply_text(input),
        Mode::ImageLuma8 => super::luma8::ops::apply_text(input),
        Mode::ImageLumaA8 => super::luma_a8::ops::apply_text(input),
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::apply_text(input)
        }
        Mode::ImagePalette8 => super::palette8::ops::apply_text(input),
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::apply_text(input),
        Mode::ImageRgb8 => super::rgb8::ops::apply_text(input),
        Mode::ImageRgba8 => super::rgba8::ops::apply_text(input),
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::apply_text(input)
        }
        Mode::ImageRgbx8 => super::rgbx8::ops::apply_text(input),
        Mode::ImageCmyk8 => super::cmyk8::ops::apply_text(input),
        Mode::ImageYCbCr8 => super::ycbcr8::ops::apply_text(input),
        Mode::ImageLab8 => super::lab8::ops::apply_text(input),
        Mode::ImageHsv8 => super::hsv8::ops::apply_text(input),
        Mode::ImageI32 => super::i32::ops::apply_text(input),
        Mode::ImageF32 => super::f32::ops::apply_text(input),
        Mode::ImageI16 => super::i16::ops::apply_text(input),
        Mode::ImageI16L => super::i16l::ops::apply_text(input),
        Mode::ImageI16B => super::i16b::ops::apply_text(input),
        Mode::ImageI16N => super::i16n::ops::apply_text(input),
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
    execute_mode(image, ops, mode, implementation.architecture)
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
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    match mode {
        Mode::One => super::one::ops::execute_with_architecture(image, ops, architecture),
        Mode::ImageLuma8 => {
            super::luma8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageLumaA8 => {
            super::luma_a8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageLaPremultiplied8 => {
            super::la_premultiplied8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImagePalette8 => {
            super::palette8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImagePaletteAlpha8 => {
            super::palette_alpha8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageRgb8 => {
            super::rgb8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageRgba8 => {
            super::rgba8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageRgbPremultiplied8 => {
            super::rgb_premultiplied8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageRgbx8 => {
            super::rgbx8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageCmyk8 => {
            super::cmyk8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageYCbCr8 => {
            super::ycbcr8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageLab8 => {
            super::lab8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageHsv8 => {
            super::hsv8::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageI32 => {
            super::i32::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageF32 => {
            super::f32::ops::execute_with_architecture(image, ops, architecture)
        }
        Mode::ImageI16 => super::i16::ops::execute(image, ops),
        Mode::ImageI16L => super::i16l::ops::execute(image, ops),
        Mode::ImageI16B => super::i16b::ops::execute(image, ops),
        Mode::ImageI16N => super::i16n::ops::execute(image, ops),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => Err(PilError::ValueError("unsupported image mode".into())),
    }
}

pub(crate) fn execute_draw_batch(
    image: &DynamicImage,
    ops: &[PipelineOp],
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    let selection = prepare_mode(image, ops, context)?;
    execute_batch(image, ops, image.mode(), selection)
}

pub(crate) fn is_draw_op(op: &PipelineOp) -> bool {
    super::helpers::is_draw_op(op)
}
