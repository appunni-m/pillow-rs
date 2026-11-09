use crate::compute::registry::support::{DrawTextMaskKind, PaletteKnowledge};
use crate::raster::DynamicImage;

/// Prepared inputs shared by exact-mode eager Text implementations.
pub(crate) struct TextComposeInput<'a> {
    /// Exact materialized destination raster.
    pub image: DynamicImage,
    /// Destination origin of the rendered glyph mask.
    pub x: i32,
    /// Destination origin of the rendered glyph mask.
    pub y: i32,
    /// Glyph-mask width in pixels.
    pub width: u32,
    /// Glyph-mask height in pixels.
    pub height: u32,
    /// Native FreeType coverage or RGBA mask bytes.
    pub pixels: &'a [u8],
    /// FreeType mask identity supplied to the support contract.
    pub mask_kind: DrawTextMaskKind,
    /// Resolved Pillow draw ink.
    pub fill: (u8, u8, u8, u8),
    /// Resolved palette state for the exact P-mode strategy.
    pub palette: PaletteKnowledge,
}

/// Result of an eager Text mode strategy.
pub(crate) enum TextComposeResult {
    /// The exact mode's strategy cannot consume the current backing variant.
    NotApplicable,
    /// The operation is a no-op and the wrapper must preserve its Image value.
    Unchanged,
    /// The strategy produced a replacement mode-bearing raster.
    Updated(DynamicImage),
}
