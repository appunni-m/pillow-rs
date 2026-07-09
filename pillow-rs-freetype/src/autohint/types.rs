//! Core data structures for the auto-hinter — port of `src/autofit/afhints.h`.
//!
//! Mirrors FreeType's `AF_PointRec`, `AF_SegmentRec`, `AF_EdgeRec`,
//! `AF_AxisHintsRec`, and `AF_GlyphHintsRec`.

use crate::outline::Outline;

// ── Direction constants (afhints.h:31–40) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i8)]
pub enum Direction {
    #[default]
    None = 0,
    Right = 1,
    Left = -1,
    Up = 2,
    Down = -2,
}

impl Direction {
    #[inline]
    pub fn is_horizontal(self) -> bool {
        matches!(self, Direction::Right | Direction::Left)
    }
    #[inline]
    pub fn is_vertical(self) -> bool {
        matches!(self, Direction::Up | Direction::Down)
    }
    #[inline]
    pub fn as_i8(self) -> i8 {
        self as i8
    }
    pub fn opposite(self) -> Direction {
        match self {
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::None => Direction::None,
        }
    }
}

// ── Dimension (afhints.h:31) ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Horz = 0, // X-axis → vertical stems/edges
    Vert = 1, // Y-axis → horizontal stems/edges
}

// ── Hinting flags (aflatin.h:152–156) ──────────────────────────────────────

pub const AF_LATIN_HINTS_HORZ_SNAP: u32 = 1 << 0;
pub const AF_LATIN_HINTS_VERT_SNAP: u32 = 1 << 1;
pub const AF_LATIN_HINTS_STEM_ADJUST: u32 = 1 << 2;
pub const AF_LATIN_HINTS_MONO: u32 = 1 << 3;

/// Scaler flag: disable horizontal hinting (set for italic fonts, light/LCD mode).
pub const AF_SCALER_FLAG_NO_HORIZONTAL: u32 = 1;

// ── Point flags (afhints.h:208–226) ────────────────────────────────────────

pub const AF_FLAG_CONIC: u16 = 1 << 0;
pub const AF_FLAG_CUBIC: u16 = 1 << 1;
pub const AF_FLAG_CONTROL: u16 = AF_FLAG_CONIC | AF_FLAG_CUBIC;
pub const AF_FLAG_TOUCH_X: u16 = 1 << 2;
pub const AF_FLAG_TOUCH_Y: u16 = 1 << 3;
pub const AF_FLAG_WEAK_INTERPOLATION: u16 = 1 << 4;
pub const AF_FLAG_NEAR: u16 = 1 << 5;
pub const AF_FLAG_IGNORE: u16 = 1 << 6;

// ── Font-wide metrics structures ─────────────────────────────────────────────
// Mirrors FreeType's AF_WidthRec, AF_LatinBlueRec, AF_LatinAxisRec, AF_LatinMetricsRec.

/// Simple (org, cur, fit) triple.  aflatin.h AF_WidthRec.
#[derive(Debug, Clone, Copy, Default)]
pub struct AfWidth {
    pub org: i32, // original (font units)
    pub cur: i32, // current (scaled 26.6)
    pub fit: i32, // fitted  (grid-aligned 26.6)
}

/// Blue zone descriptor for one vertical position (top/bottom).  aflatin.h:86
#[derive(Debug, Clone, Copy, Default)]
pub struct AfLatinBlue {
    pub ref_width: AfWidth,   // flat-segment reference
    pub shoot_width: AfWidth, // round-segment overshoot
    pub ascender: i32,
    pub descender: i32,
    pub flags: u32, // AF_LATIN_BLUE_* bits
}

/// Per-axis (Horz or Vert) font-wide metrics.  aflatin.h:97
#[derive(Debug, Clone)]
pub struct AfLatinAxisMetrics {
    pub scale: i32, // 16.16
    pub delta: i32, // 26.6
    pub width_count: usize,
    pub widths: [AfWidth; AF_LATIN_MAX_WIDTHS],
    pub edge_distance_threshold: i32, // font units
    pub standard_width: i32,
    pub extra_light: bool,
    // Vert axis only:
    pub blue_count: usize,
    pub blues: Vec<AfLatinBlue>,
    pub org_scale: i32,
    pub org_delta: i32,
}

impl Default for AfLatinAxisMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl AfLatinAxisMetrics {
    pub fn new() -> Self {
        AfLatinAxisMetrics {
            scale: 0,
            delta: 0,
            width_count: 0,
            widths: [AfWidth::default(); AF_LATIN_MAX_WIDTHS],
            edge_distance_threshold: 0,
            standard_width: 0,
            extra_light: false,
            blue_count: 0,
            blues: Vec::new(),
            org_scale: 0,
            org_delta: 0,
        }
    }
}

/// Font-wide Latin autohinter metrics.  aflatin.h:118
#[derive(Debug, Clone)]
pub struct AfLatinMetrics {
    pub units_per_em: i32,
    pub axis: [AfLatinAxisMetrics; 2], // [Horz, Vert]
    /// glyph_index → is non-base (skip blue-zone alignment).
    /// Mirrors C's globals->glyph_styles\[gindex\] & AF_NONBASE.
    pub non_base_glyphs: Vec<bool>,
    /// glyph_index → ASCII digit marker.
    /// Mirrors C's globals->glyph_styles\[gindex\] & AF_DIGIT.
    pub digit_glyphs: Vec<bool>,
    /// Whether all mapped ASCII digits share one advance width.
    /// Mirrors `style_metrics->digits_have_same_width`.
    pub digits_have_same_width: bool,
    /// Whether `FT_IS_FIXED_WIDTH(face)` is true for this face.
    pub fixed_width: bool,
    /// TOP_TO_BOTTOM hinting for Indic scripts (beng, deva, guru, goth, mong).
    /// Most scripts use bottom-to-top (false).
    pub top_to_bottom_hinting: bool,
    /// Skip x-height scale adjustment for subscript/superscript styles
    /// (latb/latp). Without HarfBuzz GSUB reshaping, the raw subscript
    /// glyph forms have wrong x-height → adjustment compresses glyph.
    pub skip_xh_adjust: bool,
    /// Disable edge-adjusted advance hinting for styles whose C hint init sets
    /// `AF_SCALER_FLAG_NO_ADVANCE`.
    ///
    /// C: `afstyles.h` maps `hani_dflt` to the CJK writing system, and
    /// `af_cjk_hints_init` always sets `AF_SCALER_FLAG_NO_ADVANCE`
    /// (`afcjk.c:1419`).  The outline can still be hinted, but pp2 is rounded
    /// from the original phantom instead of edge-adjusted.
    pub no_advance_hinting: bool,
}

impl AfLatinMetrics {
    pub fn new(upem: i32, num_glyphs: u16) -> Self {
        AfLatinMetrics {
            units_per_em: upem,
            axis: [AfLatinAxisMetrics::new(), AfLatinAxisMetrics::new()],
            non_base_glyphs: vec![false; num_glyphs as usize],
            digit_glyphs: vec![false; num_glyphs as usize],
            digits_have_same_width: true,
            fixed_width: false,
            top_to_bottom_hinting: false,
            skip_xh_adjust: false,
            no_advance_hinting: false,
        }
    }
}

/// An outline point — mirrors `AF_PointRec` (afhints.h:243–263).
#[derive(Debug, Clone, Copy, Default)]
pub struct AFPoint {
    pub flags: u16,
    pub in_dir: Direction,
    pub out_dir: Direction,
    /// Original scaled position (26.6).
    pub ox: i32,
    pub oy: i32,
    /// Original unscaled position (font units, clamped to i16).
    pub fx: i16,
    pub fy: i16,
    /// Current (hinted) position in 26.6.
    pub x: i32,
    pub y: i32,
    /// Index of next point in contour (circular).
    pub next: usize,
    /// Index of previous point in contour (circular).
    pub prev: usize,
    /// IUP scratch: hinted coordinate (u), original coordinate (v).
    pub u: i32,
    pub v: i32,
}

// ── Segment (afhints.h:266–287) ────────────────────────────────────────────

pub const AF_EDGE_ROUND: u8 = 1 << 0;
pub const AF_EDGE_NORMAL: u8 = 0;
pub const AF_EDGE_SERIF: u8 = 1 << 1;
pub const AF_EDGE_DONE: u8 = 1 << 2;
pub const AF_EDGE_NEUTRAL: u8 = 1 << 3;
pub const AF_EDGE_NO_BLUE: u8 = 1 << 4;

/// Maximum number of stem widths per axis.
pub const AF_LATIN_MAX_WIDTHS: usize = 16;

/// Blue zone property flags (from the blue stringset table).
pub const AF_BLUE_PROP_LATIN_TOP: u32 = 1 << 0;
pub const AF_BLUE_PROP_LATIN_SUB_TOP: u32 = 1 << 1;
pub const AF_BLUE_PROP_LATIN_NEUTRAL: u32 = 1 << 2;
pub const AF_BLUE_PROP_LATIN_X_HEIGHT: u32 = 1 << 3;
pub const AF_BLUE_PROP_LATIN_CAPITAL_BOTTOM: u32 = 1 << 5;
pub const AF_BLUE_PROP_LATIN_SMALL_BOTTOM: u32 = 1 << 6;

/// Blue zone flags (runtime, stored on AF_LatinBlue.flags).
pub const AF_LATIN_BLUE_ACTIVE: u32 = 1 << 0;
pub const AF_LATIN_BLUE_TOP: u32 = 1 << 1;
pub const AF_LATIN_BLUE_SUB_TOP: u32 = 1 << 2;
pub const AF_LATIN_BLUE_NEUTRAL: u32 = 1 << 3;
pub const AF_LATIN_BLUE_ADJUSTMENT: u32 = 1 << 4;
pub const AF_LATIN_BLUE_BOTTOM: u32 = 1 << 5;
pub const AF_LATIN_BLUE_BOTTOM_SMALL: u32 = 1 << 6;

#[derive(Debug, Clone, Copy, Default)]
pub struct AFSegment {
    pub flags: u8,
    pub dir: Direction,
    /// Position along the main axis (font units, i16).
    pub pos: i16,
    /// Deviation from pos.
    pub delta: i16,
    /// Min coordinate on the cross-axis (font units).
    pub min_coord: i16,
    /// Max coordinate on the cross-axis.
    pub max_coord: i16,
    /// Segment height (max_coord - min_coord), used for edge filtering.
    pub height: i16,
    /// First point index.
    pub first: usize,
    /// Last point index.
    pub last: usize,
    /// Parent edge index (or usize::MAX).
    pub edge: usize,
    /// Next segment in the same edge (usize::MAX = end).
    pub edge_next: usize,
    /// Stem link: paired segment (usize::MAX = none).
    pub link: usize,
    /// Serif link (usize::MAX = none).
    pub serif: usize,
    pub score: i32,
}

// ── Edge (afhints.h:290–308) ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct AFEdge {
    /// Position in font units (canonical key).
    pub fpos: i16,
    /// Original scaled position (26.6).
    pub opos: i32,
    /// Current (grid-fitted) position (26.6).
    pub pos: i32,
    pub flags: u8,
    pub dir: Direction,
    /// Stem link (paired edge).
    pub link: usize,
    /// Serif edge.
    pub serif: usize,
    /// First segment in this edge.
    pub first: usize,
    /// Last segment in this edge.
    pub last: usize,
    /// Blue zone reference (fitted position to snap to), if any.
    pub blue_edge: Option<AfWidth>,
}

impl Default for AFEdge {
    fn default() -> Self {
        AFEdge {
            fpos: 0,
            opos: 0,
            pos: 0,
            flags: 0,
            dir: Direction::None,
            link: usize::MAX,
            serif: usize::MAX,
            first: usize::MAX,
            last: usize::MAX,
            blue_edge: None,
        }
    }
}

// ── Axis hints (afhints.h:314–334) ────────────────────────────────────────

/// Per-dimension hints: segments + edges for Horz (X) or Vert (Y).
#[derive(Debug, Clone)]
pub struct AxisHints {
    pub segments: Vec<AFSegment>,
    pub edges: Vec<AFEdge>,
    pub major_dir: Direction,
}

impl Default for AxisHints {
    fn default() -> Self {
        Self::new()
    }
}

impl AxisHints {
    pub fn new() -> Self {
        AxisHints {
            segments: Vec::new(),
            edges: Vec::new(),
            major_dir: Direction::None,
        }
    }
}

// ── Glyph hints (afhints.h:340–377) ───────────────────────────────────────

/// Top-level hinting state for a single glyph.
#[derive(Debug, Clone)]
pub struct GlyphHints {
    pub x_scale: i32, // 16.16
    pub y_scale: i32,
    pub x_delta: i32, // 26.6
    pub y_delta: i32,

    pub points: Vec<AFPoint>,
    pub contours: Vec<usize>, // index of first point of each contour
    pub contour_y_minima: Vec<i32>,
    pub contour_y_maxima: Vec<i32>,

    pub axis: [AxisHints; 2],

    /// Points-per-em for debug
    pub ppem: i32,

    /// Hinting control flags (aflatin.h:152-156).
    pub other_flags: u32,

    /// Scaler flags (e.g., AF_SCALER_FLAG_NO_HORIZONTAL for italic).
    pub scaler_flags: u32,

    /// Font-wide Latin metrics (stem widths, blue zones).  Owned clone.
    pub metrics: Option<AfLatinMetrics>,

    /// Glyph outline orientation: true = clockwise (PostScript).
    pub cw_orientation: bool,
}

impl GlyphHints {
    pub fn new(x_scale: i32, y_scale: i32, x_delta: i32, y_delta: i32) -> Self {
        GlyphHints {
            x_scale,
            y_scale,
            x_delta,
            y_delta,
            points: Vec::new(),
            contours: Vec::new(),
            contour_y_minima: Vec::new(),
            contour_y_maxima: Vec::new(),
            axis: [AxisHints::new(), AxisHints::new()],
            ppem: 0,
            other_flags: 0,
            scaler_flags: 0,
            metrics: None,
            cw_orientation: false,
        }
    }

    /// Number of contours.
    pub fn num_contours(&self) -> usize {
        self.contours.len()
    }

    /// Number of points.
    pub fn num_points(&self) -> usize {
        self.points.len()
    }

    /// Copy hinted coordinates back into an Outline.
    pub fn save_to_outline(&self, outline: &mut Outline) {
        for (i, pt) in self.points.iter().enumerate() {
            if let Some(op) = outline.points.get_mut(i) {
                op.x = pt.x;
                op.y = pt.y;
            }
        }
    }
}
