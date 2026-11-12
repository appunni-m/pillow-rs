//! Resize operation contracts and prepared mode-strategy selection.
//!
//! Boxed Resize keeps Pillow's float32 bounds, crop/pass-elision, public
//! validation order, and vertical-first tall-image composition. Public and
//! nested geometry callers enter prepared plans with the injected context.
//! One and P normalize to nearest before mode and architecture selection;
//! their Default routes remain serial fallbacks when no vector route applies.
//! Rayon scheduling belongs to Parallel CPU.
//! TODO(pillow-rs#3ptb): verify exact-mode output, bounds/empty cases, and
//! requested-architecture fallback receipts across the frozen CPU matrix.
//! Resize/ResizeBoxed now compose omitted lanes at the prepared execution
//! boundary. Current fused Resize-to-Pad owners are L and RGBX; neither has
//! omitted physical lanes. Compact modes use prepared Resize followed by Pad.

use crate::compute::registry::support::{ModeSupport, OperationId, OperationSupport};
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::SUPPORT,
        Mode::ImageI32 => super::i32::ops::SUPPORT,
        Mode::ImageF32 => super::f32::ops::SUPPORT,
        Mode::ImageI16 => super::i16::ops::SUPPORT,
        Mode::ImageI16L => super::i16l::ops::SUPPORT,
        Mode::ImageI16B => super::i16b::ops::SUPPORT,
        Mode::ImageI16N => super::i16n::ops::SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::SUPPORT,
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn classify_boxed_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::one::ops::BOXED_SUPPORT,
        Mode::ImageLuma8 => super::luma8::ops::BOXED_SUPPORT,
        Mode::ImageLumaA8 => super::luma_a8::ops::BOXED_SUPPORT,
        Mode::ImageLaPremultiplied8 => super::la_premultiplied8::ops::BOXED_SUPPORT,
        Mode::ImagePalette8 => super::palette8::ops::BOXED_SUPPORT,
        Mode::ImagePaletteAlpha8 => super::palette_alpha8::ops::BOXED_SUPPORT,
        Mode::ImageRgb8 => super::rgb8::ops::BOXED_SUPPORT,
        Mode::ImageRgba8 => super::rgba8::ops::BOXED_SUPPORT,
        Mode::ImageRgbPremultiplied8 => super::rgb_premultiplied8::ops::BOXED_SUPPORT,
        Mode::ImageRgbx8 => super::rgbx8::ops::BOXED_SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::BOXED_SUPPORT,
        Mode::ImageYCbCr8 => super::ycbcr8::ops::BOXED_SUPPORT,
        Mode::ImageLab8 => super::lab8::ops::BOXED_SUPPORT,
        Mode::ImageHsv8 => super::hsv8::ops::BOXED_SUPPORT,
        Mode::ImageI32 => super::i32::ops::BOXED_SUPPORT,
        Mode::ImageF32 => super::f32::ops::BOXED_SUPPORT,
        Mode::ImageI16 => super::i16::ops::BOXED_SUPPORT,
        Mode::ImageI16L => super::i16l::ops::BOXED_SUPPORT,
        Mode::ImageI16B => super::i16b::ops::BOXED_SUPPORT,
        Mode::ImageI16N => super::i16n::ops::BOXED_SUPPORT,
        Mode::ImageLumaA16 => super::luma_a16::ops::BOXED_SUPPORT,
        Mode::ImageRgb16 => super::rgb16::ops::BOXED_SUPPORT,
        Mode::ImageRgba16 => super::rgba16::ops::BOXED_SUPPORT,
        Mode::ImageRgb32F => super::rgb32f::ops::BOXED_SUPPORT,
        Mode::ImageRgba32F => super::rgba32f::ops::BOXED_SUPPORT,
    }
}

/// Exhaustive source-mode support for ordinary Resize.
pub(crate) static SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::Resize,
    super::helpers::support::INPUTS,
    classify_resize,
);

/// Exhaustive source-mode support for boxed Resize.
pub(crate) static BOXED_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ResizeBoxed,
    super::helpers::support::INPUTS,
    classify_boxed_resize,
);

/// Resolve exact-mode filter policy before selecting or preparing a kernel.
#[path = "helpers/prepared.rs"]
mod prepared;
pub(crate) use prepared::{
    PreparedResize, alpha_reducing_gap_support, effective_filter, prepare_direct,
    prepare_direct_with_reducing_gap_selection, prepare_direct_with_selection, prepare_pipeline,
};
