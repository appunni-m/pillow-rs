//! Error types for pillow-rs-freetype.
//!
//! Mirrors FreeType's `FT_Error` categories that are reachable from the
//! rendering path we port.

use thiserror::Error;

/// Errors that can occur during font loading, glyph lookup, or rendering.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum FontError {
    /// The font data is not a valid TrueType/OpenType font.
    #[error("Invalid TrueType font: {0}")]
    InvalidFont(String),

    /// The cmap table uses an unsupported format.
    #[error("Unsupported cmap table format: {0}")]
    UnsupportedCmapFormat(u16),

    /// The rasterizer render pool overflowed (FreeType `Raster_Overflow`).
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,

    /// Glyph outline data is malformed.
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),
}
