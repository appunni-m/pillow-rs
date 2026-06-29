//! Code-coverage tracking for the Latin autohinter.
//!
//! Uses a thread-local `u64` bitmask to record which code paths are
//! exercised during processing of a single glyph.  After rendering,
//! callers can read the accumulated bitmask and reset it for the next
//! glyph.
//!
//! Synced with `latin.rs` — each `COV_*` constant documents one
//! decision point in the hinting pipeline.

use std::cell::RefCell;

thread_local! {
    /// Accumulated coverage bits for the glyph currently being processed.
    static COVERAGE_MASK: RefCell<u64> = RefCell::new(0);
}

/// Macro to record a code-path hit at the call site.
/// Usage: cov_hit!(COV_SEGMENTS_DIR);
#[macro_export]
macro_rules! cov_hit {
    ($bit:expr) => {
        $crate::autohint::coverage::record($bit);
    };
}

// ── Coverage bit definitions ──────────────────────────────────────────────

/// compute_segments: direction assignment occurred.
pub const COV_SEGMENTS_DIR: u64 = 1 << 0;
/// compute_edges: segment filtered by height check.
pub const COV_EDGES_HEIGHT_FILTER: u64 = 1 << 1;
/// compute_edges: serif link detected.
pub const COV_EDGES_SERIF_LINK: u64 = 1 << 2;
/// compute_edges: segment link detected (non-serif).
pub const COV_EDGES_SEGMENT_LINK: u64 = 1 << 3;
/// compute_blue_edges: capital-top blue zone assigned.
pub const COV_BLUE_CAPITAL_TOP: u64 = 1 << 4;
/// compute_blue_edges: capital-bottom blue zone assigned.
pub const COV_BLUE_CAPITAL_BOTTOM: u64 = 1 << 5;
/// compute_blue_edges: small-top blue zone assigned.
pub const COV_BLUE_SMALL_TOP: u64 = 1 << 6;
/// compute_blue_edges: small-bottom blue zone assigned.
pub const COV_BLUE_SMALL_BOTTOM: u64 = 1 << 7;
/// compute_blue_edges: neutral blue zone used.
pub const COV_BLUE_NEUTRAL: u64 = 1 << 8;
/// hint_edges Phase 1: blue-zone alignment applied.
pub const COV_HINT_PHASE1_BLUE: u64 = 1 << 9;
/// hint_edges Phase 2: anchor stem alignment.
pub const COV_HINT_PHASE2_ANCHOR: u64 = 1 << 10;
/// hint_edges Phase 2: relative stem alignment.
pub const COV_HINT_PHASE2_RELATIVE: u64 = 1 << 11;
/// hint_edges Phase 2: BOUND check triggered.
pub const COV_HINT_PHASE2_BOUND: u64 = 1 << 12;
/// hint_edges Phase 3: 6-edge lowercase-m symmetry.
pub const COV_HINT_PHASE3_6EDGE: u64 = 1 << 13;
/// hint_edges Phase 3: 12-edge lowercase-m symmetry.
pub const COV_HINT_PHASE3_12EDGE: u64 = 1 << 14;
/// hint_edges Phase 4: serif handling.
pub const COV_HINT_PHASE4_SERIF: u64 = 1 << 15;
/// hint_edges Phase 4: anchor-relative adjustment.
pub const COV_HINT_PHASE4_ANCHOR_REL: u64 = 1 << 16;
/// hint_edges Phase 4: interpolation adjustment.
pub const COV_HINT_PHASE4_INTERP: u64 = 1 << 17;
/// compute_stem_width: serif short-circuit path.
pub const COV_STEM_SERIF: u64 = 1 << 18;
/// compute_stem_width: round-edge path.
pub const COV_STEM_ROUND: u64 = 1 << 19;
/// compute_stem_width: thin clamp applied.
pub const COV_STEM_THIN: u64 = 1 << 20;
/// compute_stem_width: standard-width match.
pub const COV_STEM_STANDARD: u64 = 1 << 21;
/// compute_stem_width: fractional quantisation.
pub const COV_STEM_FRAC: u64 = 1 << 22;
/// compute_stem_width: bdelta applied.
pub const COV_STEM_BDELTA: u64 = 1 << 23;
/// align_strong_points: before-first-edge interpolation.
pub const COV_STRONG_BEFORE_FIRST: u64 = 1 << 24;
/// align_strong_points: after-last-edge interpolation.
pub const COV_STRONG_AFTER_LAST: u64 = 1 << 25;
/// align_strong_points: exact edge match.
pub const COV_STRONG_EXACT: u64 = 1 << 26;
/// align_strong_points: between-edges interpolation.
pub const COV_STRONG_INTERP: u64 = 1 << 27;
/// iup_shift / iup_interp: single-reference shift applied.
pub const COV_IUP_SHIFT: u64 = 1 << 28;
/// iup_shift / iup_interp: dual-reference interpolation applied.
pub const COV_IUP_INTERP: u64 = 1 << 29;
/// vertical_separation_adjustments: adjustment applied.
pub const COV_VSEP_APPLIED: u64 = 1 << 30;
/// vertical_separation_adjustments: adjustment not applied.
pub const COV_VSEP_NOT_APPLIED: u64 = 1 << 31;
/// Italic: NO_HORIZONTAL path taken.
pub const COV_ITALIC_NO_HORZ: u64 = 1 << 32;
/// Italic: horizontal hinting skipped.
pub const COV_ITALIC_HORZ_SKIPPED: u64 = 1 << 33;
/// Non-base glyph detected (accent/diacritic).
pub const COV_NONBASE_GLYPH: u64 = 1 << 34;
/// Extra-light / thin font detected.
pub const COV_EXTRA_LIGHT: u64 = 1 << 35;

/// Record a coverage bit for the current glyph.
pub fn record(bit: u64) {
    COVERAGE_MASK.with(|mask| {
        *mask.borrow_mut() |= bit;
    });
}

/// Read the accumulated coverage bits for the current glyph.
pub fn current_mask() -> u64 {
    COVERAGE_MASK.with(|mask| *mask.borrow())
}

/// Reset the coverage accumulator (call before each new glyph).
pub fn reset() {
    COVERAGE_MASK.with(|mask| {
        *mask.borrow_mut() = 0;
    });
}

/// Collect all coverage bits that have been hit at least once across the
/// entire rendering pass.  Returns a vector of unique bit positions.
pub fn collect_hit_bits(accumulated: u64) -> Vec<u32> {
    let mut bits = Vec::new();
    let mut mask = accumulated;
    let mut pos = 0u32;
    while mask != 0 {
        if mask & 1 != 0 {
            bits.push(pos);
        }
        mask >>= 1;
        pos += 1;
    }
    bits
}
