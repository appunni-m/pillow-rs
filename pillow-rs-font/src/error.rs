//! Font error types.

/// Errors that can occur during font loading, glyph lookup, or rendering.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
    /// The font data is not a valid TrueType/OpenType font.
    #[error("Invalid TrueType font: {0}")]
    InvalidFont(String),

    /// The cmap table uses an unsupported format.
    #[error("Unsupported cmap table format: {0}")]
    UnsupportedCmapFormat(u16),

    /// The rasterizer ran out of buffer space.
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,

    /// Glyph outline data is malformed.
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),
}
