//! Auto-hinting module — ports FreeType's `autofit` engine for Latin script.
//!
//! ## Why auto-hinting exists
//!
//! A TrueType outline describes a glyph as contour points in font units (FU).
//! At small sizes (10-24pt), the outline maps to only a handful of screen pixels.
//! Without hinting: diagonal strokes blur, vertical stems have different widths,
//! round parts look asymmetric.
//!
//! The auto-hinter analyzes outline geometry and **snaps edges to pixel
//! boundaries** while preserving proportions. Each dimension (X/Y) is processed
//! independently with its own edges, segments, and alignment phases.
//!
//! ## Pipeline order (why this sequence)
//!
//! ```text
//! RELOAD:      Load coords, compute direction vectors, build direction chain.
//!              The direction chain merges smooth curves into single segments
//!              so compute_segments doesn't fragment them.
//!   ↓
//! SEGMENTS:    Find horizontal/vertical runs — groups of consecutive points
//!              moving in the same direction.
//!   ↓
//! EDGES:       Merge overlapping segments into edges. The left side of an 'H'
//!              might be 3 segments (serif+stem+serif) — they must merge into
//!              ONE edge that snaps to ONE pixel column.
//!   ↓
//! HINT EDGES:  4-phase snapping: (1) stem pairs to integer pixels, (2) serifs
//!              to linked stems, (3) blue zones (baseline, cap-height),
//!              (4) anchor propagation to unaligned edges.
//!   ↓
//! ALIGN:       align_edge: snap contour points to hinted edges.
//!              align_strong: interpolate corner points between edges.
//!              align_weak (IUP): interpolate smooth runs between corners.
//!   ↓
//! PHANTOM:     Shift glyph to pixel grid using pp1.x from edge positions.
//!   ↓
//! RASTERIZE:   Convert hinted outline → 8-bit alpha bitmap (DDA stepping).
//! ```
//!
//! ## The WEAK/STRONG classification (the most subtle part)
//!
//! After the direction chain runs, each point is classified as STRONG (corner,
//! needs explicit grid-fitting) or WEAK (on a straight/flat run, gets
//! interpolated by IUP). Getting this wrong changes which points serve as
//! IUP reference anchors, producing 1-2 unit coordinate drift across entire
//! contour sections.
//!
//! The "both-None" case (in_dir==out_dir==None) runs two sequential tests:
//! 1. XOR quadrant check: same sign on both axes? → WEAK
//! 2. corner_is_flat: one vector dominates? → WEAK **and** update direction-
//!    chain deltas (pv→u, nu→v) that affect downstream classifications.
//!
//! If test 2's delta update is skipped (e.g. by OR-ing the two checks into
//! one boolean), downstream points see old u/v values, get different WEAK
//! flags, and the IUP uses different reference pairs.
//!
//! ## Font category behavior
//!
//! | Category | near_limit | Behavior |
//! |----------|-----------|----------|
//! | UPEM=2048 upright | 20 FU | Sparse direction chain, straightforward classification |
//! | UPEM=2048 italic | 20 FU | NO HORIZONTAL (skips X-axis hinting) |
//! | UPEM=1000 bold | 9 FU | Dense direction chain, more points merge, classification is fragile |
//! | Liberation bold/mono/narrow | 20 FU | Different stem-width thresholds affect edge grouping |
//!
//! Reference: `freetype/src/autofit/` (VER-2-14-1).
//! Algorithm reference: `ALGORITHMS.md` in this directory.

pub mod types;
pub mod coverage;
pub mod loader;
pub mod latin;

pub use latin::apply_hints;
pub use types::{GlyphHints, AxisHints, AFPoint, AFSegment, AFEdge, Direction, Dimension,
    AfWidth, AfLatinBlue, AfLatinAxisMetrics, AfLatinMetrics,
    AF_LATIN_MAX_WIDTHS,
};
