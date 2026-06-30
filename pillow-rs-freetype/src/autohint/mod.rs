//! Auto-hinter: snaps glyph edges to pixel grid for readability at small sizes.
//!
//! # Pipeline (per dimension, HORZ then VERT)
//!
//! 1. `reload` → coords + direction chain + WEAK/STRONG classify
//! 2. `compute_segments` → horizontal/vertical runs
//! 3. `compute_edges` → merge overlapping segments
//! 4. `compute_blue_edges` → assign to baseline/cap-height zones
//! 5. `hint_edges` → 4-phase snap: stems → serifs → blues → anchors
//! 6. `align_edge_points` → snap contour points to hinted edges
//! 7. `align_strong_points` → grid-fit corners (skips WEAK)
//! 8. `align_weak_points` (IUP) → interpolate smooth runs
//! 9. phantom adjust → pixel-grid shift via pp1.x
//!
//! # WEAK/STRONG classification
//!
//! See `reload` and `build_direction_chain` in `loader.rs`. Wrong flag here
//! cascades: skipped point → wrong IUP ref → 1-2 unit drift → pixel mismatch.
//!
//! # Font categories
//!
//! | Category | `near_limit` | Behavior |
//! |----------|-------------|----------|
//! | UPEM=2048 | 20 FU | Sparse chain, classification clear |
//! | UPEM=1000 | 9 FU | Dense chain, more merges, fragile |
//! | Italic | 20 FU | NO_HORIZONTAL (skips X-axis) |
//!
//! Reference: `freetype/src/autofit/` (VER-2-14-1).

pub mod types;
pub mod coverage;
pub mod loader;
pub mod latin;

pub use latin::apply_hints;
pub use types::{GlyphHints, AxisHints, AFPoint, AFSegment, AFEdge, Direction, Dimension,
    AfWidth, AfLatinBlue, AfLatinAxisMetrics, AfLatinMetrics,
    AF_LATIN_MAX_WIDTHS,
};
