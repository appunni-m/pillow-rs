//! Auto-hinting module — ports FreeType's `autofit` engine for Latin script.
//!
//! The auto-hinter modifies glyph outline coordinates to align edges to the
//! pixel grid, improving readability at small sizes. Unlike bytecode hinting
//! (which interprets TrueType instructions), auto-hinting works purely from
//! the outline geometry.
//!
//! Reference: `freetype/src/autofit/` (VER-2-14-1).

pub mod types;
pub mod loader;
pub mod latin;

pub use latin::apply_hints;
pub use types::{GlyphHints, AxisHints, AFPoint, AFSegment, AFEdge, Direction, Dimension,
    AfWidth, AfLatinBlue, AfLatinAxisMetrics, AfLatinMetrics,
    AF_LATIN_MAX_WIDTHS,
};
