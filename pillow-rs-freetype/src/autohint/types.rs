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

// ── Point flags (afhints.h:208–226) ────────────────────────────────────────

pub const AF_FLAG_CONIC: u16 = 1 << 0;
pub const AF_FLAG_CUBIC: u16 = 1 << 1;
pub const AF_FLAG_CONTROL: u16 = AF_FLAG_CONIC | AF_FLAG_CUBIC;
pub const AF_FLAG_TOUCH_X: u16 = 1 << 2;
pub const AF_FLAG_TOUCH_Y: u16 = 1 << 3;
pub const AF_FLAG_WEAK_INTERPOLATION: u16 = 1 << 4;
pub const AF_FLAG_NEAR: u16 = 1 << 5;
pub const AF_FLAG_IGNORE: u16 = 1 << 6;

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
pub const AF_EDGE_SERIF: u8 = 1 << 1;
pub const AF_EDGE_DONE: u8 = 1 << 2;

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
}

impl Default for AFEdge {
    fn default() -> Self {
        AFEdge {
            fpos: 0, opos: 0, pos: 0,
            flags: 0, dir: Direction::None,
            link: usize::MAX, serif: usize::MAX,
            first: usize::MAX, last: usize::MAX,
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
