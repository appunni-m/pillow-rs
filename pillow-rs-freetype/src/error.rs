//! Error types for `freetype`.
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

    /// The rasterizer render pool overflowed (FreeType `Raster_Overflow`).
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,

    /// Glyph outline data is malformed.
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),

    /// TrueType bytecode exceeded FreeType's runnable instruction limit.
    #[error("TrueType bytecode execution too long")]
    ExecutionTooLong,

    /// Pedantic TrueType bytecode referenced a point outside its active zone.
    #[error("Invalid TrueType bytecode point reference")]
    InvalidReference,

    /// The loaded glyph slot format cannot be rendered.
    #[error("Cannot render glyph: {0}")]
    CannotRenderGlyph(String),

    /// The requested FreeType-style argument combination is invalid.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// The selected embedded bitmap strike has no image for the glyph.
    #[error("Missing embedded bitmap")]
    MissingBitmap,

    /// A TrueType outline or embedded bitmap composite is malformed.
    #[error("Invalid glyph composite")]
    InvalidComposite,
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
    fn raster_overflow_has_static_message() {
        let err = FontError::RasterOverflow;
        assert_eq!(err.to_string(), "Rasterizer buffer overflow");
    }

    #[test]
    fn invalid_outline_displays_message() {
        let err = FontError::InvalidOutline("bad contour".into());
        assert_eq!(err.to_string(), "Invalid glyph outline: bad contour");
    }

    #[test]
    fn execution_too_long_has_static_message() {
        let err = FontError::ExecutionTooLong;
        assert_eq!(err.to_string(), "TrueType bytecode execution too long");
    }

    #[test]
    fn invalid_reference_has_static_message() {
        let err = FontError::InvalidReference;
        assert_eq!(err.to_string(), "Invalid TrueType bytecode point reference");
    }

    #[test]
    fn cannot_render_glyph_displays_message() {
        let err = FontError::CannotRenderGlyph("composite slot".into());
        assert_eq!(err.to_string(), "Cannot render glyph: composite slot");
    }

    #[test]
    fn invalid_argument_displays_message() {
        let err = FontError::InvalidArgument("missing strike".into());
        assert_eq!(err.to_string(), "Invalid argument: missing strike");
    }

    #[test]
    fn missing_bitmap_has_static_message() {
        let err = FontError::MissingBitmap;
        assert_eq!(err.to_string(), "Missing embedded bitmap");
    }

    #[test]
    fn invalid_composite_has_static_message() {
        let err = FontError::InvalidComposite;
        assert_eq!(err.to_string(), "Invalid glyph composite");
    }
}
