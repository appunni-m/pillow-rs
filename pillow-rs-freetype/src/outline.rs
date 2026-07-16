//! Scaled glyph outline in 26.6 fixed-point — the input to the smooth rasterizer.
//!
//! Represents FreeType's `FT_Outline` at the point it is handed to
//! `ft_gray_raster.raster_render`: coordinates in 26.6, contours as endpoint
//! indices, and per-point on/off-curve tags.

/// A point in an `FT_Outline`-style outline (26.6 coordinates).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutlinePoint {
    pub x: i32,
    pub y: i32,
    pub on_curve: bool,
}

pub(crate) const OUTLINE_HIGH_PRECISION: u32 = 0x100;
pub(crate) const OUTLINE_SINGLE_PASS: u32 = 0x200;
pub(crate) const OUTLINE_REVERSE_FILL: u32 = 0x4;
pub(crate) const OUTLINE_IGNORE_DROPOUTS: u32 = 0x8;
pub(crate) const OUTLINE_SMART_DROPOUTS: u32 = 0x10;
pub(crate) const OUTLINE_INCLUDE_STUBS: u32 = 0x20;
pub(crate) const OUTLINE_OVERLAP: u32 = 0x40;

/// FreeType's `FT_Outline`: flattened contours in 26.6 units.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outline {
    /// Number of contours.
    pub n_contours: i32,
    /// Endpoint index of each contour.
    pub contours: Vec<i16>,
    /// Flattened points across all contours.
    pub points: Vec<OutlinePoint>,
    /// Full FreeType outline tag bytes when a loader has exact public tags.
    /// Empty means derive curve tags from [`OutlinePoint::on_curve`].
    pub tags: Vec<u8>,
    /// Per-contour black rasterizer dropout controls.  Empty means derive the
    /// control from [`Self::flags`].
    pub contour_dropouts: Vec<u8>,
    /// Outline flags (`FT_OUTLINE_EVEN_ODD_FILL` etc.). TrueType uses the
    /// default non-zero fill.
    pub flags: u32,
    /// Pixel-aligned CBox used to size the target bitmap (integer pixel coords).
    pub cbox_x_min: i32,
    pub cbox_y_min: i32,
    pub cbox_x_max: i32,
    pub cbox_y_max: i32,
}

impl Outline {
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() || self.n_contours == 0
    }
}
