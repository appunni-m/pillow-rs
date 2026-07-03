//! Glyph zone — the coordinate working area for bytecode hinting.
//!
//! C reference: `TT_GlyphZoneRec` in `tttypes.h:1643`,
//! `tt_prepare_zone` in `ttgload.c:751-767`.
//!
//! A zone holds three coordinate arrays:
//! - `org`: original coordinates in 26.6 (set from scaled outline)
//! - `cur`: current coordinates in 26.6 (modified by hinting)
//! - `orus`: original unscaled coordinates in font units
//!
//! The bytecode interpreter reads from `org`/`orus` and writes to `cur`.
//! After hinting completes, `cur` contains the grid-fitted positions.

/// A glyph zone containing coordinate arrays for a hinted glyph.
///
/// Decomposed into separate x/y arrays for better cache locality
/// and to avoid (i32, i32) tuple overhead in the hot VM loop.
///
/// The zone includes 4 phantom points at indices [n_points..n_points+3].
#[derive(Debug, Clone)]
pub struct GlyphZone {
    /// Current x coordinates in 26.6 (modified by the interpreter)
    pub cur_x: Vec<i32>,
    /// Current y coordinates in 26.6 (modified by the interpreter)
    pub cur_y: Vec<i32>,

    /// Original x coordinates in 26.6 (snapshot before hinting)
    pub org_x: Vec<i32>,
    /// Original y coordinates in 26.6 (snapshot before hinting)
    pub org_y: Vec<i32>,

    /// Unscaled original x coordinates in font units
    pub orus_x: Vec<i32>,
    /// Unscaled original y coordinates in font units
    pub orus_y: Vec<i32>,

    /// Touch flags: bit 0 = TOUCH_X, bit 1 = TOUCH_Y, bit 2 = TOUCH_BOTH
    pub tags: Vec<u8>,

    /// Contour end point indices (from glyf table)
    pub contours: Vec<u16>,

    /// Total point count (including phantom points)
    pub n_points: u16,

    /// Number of contours
    pub n_contours: u16,

    /// Offset of first point in the current sub-glyph (for composites)
    pub first_point: u16,
}

impl GlyphZone {
    /// Get the current (x, y) of a point as a tuple.
    #[inline]
    pub fn cur(&self, idx: usize) -> (i32, i32) {
        (self.cur_x[idx], self.cur_y[idx])
    }

    /// Set the current (x, y) of a point.
    #[inline]
    pub fn set_cur(&mut self, idx: usize, x: i32, y: i32) {
        self.cur_x[idx] = x;
        self.cur_y[idx] = y;
    }

    /// Get the original (x, y) of a point as a tuple.
    #[inline]
    pub fn org(&self, idx: usize) -> (i32, i32) {
        (self.org_x[idx], self.org_y[idx])
    }

    /// Get the unscaled original (x, y) of a point as a tuple.
    #[inline]
    pub fn orus(&self, idx: usize) -> (i32, i32) {
        (self.orus_x[idx], self.orus_y[idx])
    }

    /// Get the tag byte for a point.
    #[inline]
    pub fn tag(&self, idx: usize) -> u8 {
        self.tags[idx]
    }

    /// Set the tag byte for a point.
    #[inline]
    pub fn set_tag(&mut self, idx: usize, tag: u8) {
        self.tags[idx] = tag;
    }

    /// Number of real points (excluding phantom points).
    pub fn n_real_points(&self) -> usize {
        (self.n_points as usize).saturating_sub(4)
    }
}
