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

    /// A required SFNT subtable is structurally malformed.
    #[error("Invalid font table: {0}")]
    InvalidTable(String),

    /// A public FreeType-style array allocation request is too large.
    #[error("Array allocation size too large")]
    ArrayTooLarge,

    /// The rasterizer render pool overflowed (FreeType `Raster_Overflow`).
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,

    /// Glyph outline data is malformed.
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),

    /// TrueType bytecode exceeded FreeType's runnable instruction limit.
    #[error("TrueType bytecode execution too long")]
    ExecutionTooLong,

    /// TrueType bytecode tried to fetch past the active code range.
    #[error("TrueType bytecode code range overflow")]
    CodeOverflow,

    /// Pedantic TrueType bytecode referenced a point outside its active zone.
    #[error("Invalid TrueType bytecode point reference")]
    InvalidReference,

    /// The loaded glyph slot format cannot be rendered.
    #[error("Cannot render glyph: {0}")]
    CannotRenderGlyph(String),

    /// The requested operation is valid but the selected renderer does not
    /// implement the source format.
    #[error("Unimplemented feature: {0}")]
    UnimplementedFeature(String),

    /// The requested FreeType-style argument combination is invalid.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// The selected embedded bitmap strike has no image for the glyph.
    #[error("Missing embedded bitmap")]
    MissingBitmap,

    /// A TrueType outline or embedded bitmap composite is malformed.
    #[error("Invalid glyph composite")]
    InvalidComposite,

    /// BDF-like input did not start with `STARTFONT` at public face open.
    #[error("BDF stream is missing STARTFONT")]
    BdfMissingStartfontStreamOperation,

    /// SFNT offset table was readable but exposed no usable table records.
    #[error("SFNT stream has no table records")]
    SfntZeroTablesStreamOperation,

    /// A BDF glyph bitmap declaration is too large.
    #[error("BDF glyph bitmap is too large")]
    BdfBbxTooBig,

    /// BDF header fields are structurally corrupted or incomplete.
    #[error("BDF font header is corrupted")]
    BdfCorruptedFontHeader,

    /// BDF glyph fields are structurally corrupted or incomplete.
    #[error("BDF font glyphs are corrupted")]
    BdfCorruptedFontGlyphs,

    /// BDF glyph is missing its `BBX` field.
    #[error("BDF glyph is missing BBX field")]
    BdfMissingBbxField,

    /// BDF glyph is missing its `ENCODING` field.
    #[error("BDF glyph is missing ENCODING field")]
    BdfMissingEncodingField,

    /// BDF header is missing its `FONT` field.
    #[error("BDF header is missing FONT field")]
    BdfMissingFontField,

    /// BDF header is missing its `FONTBOUNDINGBOX` field.
    #[error("BDF header is missing FONTBOUNDINGBOX field")]
    BdfMissingFontboundingboxField,

    /// BDF header is missing its `SIZE` field.
    #[error("BDF header is missing SIZE field")]
    BdfMissingSizeField,

    /// BDF glyph data is missing a valid `STARTCHAR` section.
    #[error("BDF glyph is missing STARTCHAR field")]
    BdfMissingStartcharField,
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
    fn invalid_table_displays_message() {
        let err = FontError::InvalidTable("bad CFF INDEX".into());
        assert_eq!(err.to_string(), "Invalid font table: bad CFF INDEX");
    }

    #[test]
    fn array_too_large_has_static_message() {
        let err = FontError::ArrayTooLarge;
        assert_eq!(err.to_string(), "Array allocation size too large");
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
    fn code_overflow_has_static_message() {
        let err = FontError::CodeOverflow;
        assert_eq!(err.to_string(), "TrueType bytecode code range overflow");
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
    fn unimplemented_feature_displays_message() {
        let err = FontError::UnimplementedFeature("packed bitmap SDF".into());
        assert_eq!(err.to_string(), "Unimplemented feature: packed bitmap SDF");
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

    #[test]
    fn bdf_missing_startfont_has_static_message() {
        let err = FontError::BdfMissingStartfontStreamOperation;
        assert_eq!(err.to_string(), "BDF stream is missing STARTFONT");
    }
}
