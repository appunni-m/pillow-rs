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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_font_displays_message() {
        let err = FontError::InvalidFont("bad table".into());
        assert_eq!(err.to_string(), "Invalid TrueType font: bad table");
    }

    #[test]
    fn unsupported_cmap_displays_format() {
        let err = FontError::UnsupportedCmapFormat(42);
        assert_eq!(err.to_string(), "Unsupported cmap table format: 42");
    }

    #[test]
    fn raster_overflow_has_static_message() {
        let err = FontError::RasterOverflow;
        assert_eq!(err.to_string(), "Rasterizer buffer overflow");
    }

    #[test]
    fn invalid_outline_displays_message() {
        let err = FontError::InvalidOutline("bad contour".into());
        assert_eq!(err.to_string(), "Invalid glyph outline: bad contour");
    }
}
