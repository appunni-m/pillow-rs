//! Parsed font tables held in `Arc<FontData>` for cheap sharing.
//!
//! `Font` holds `Arc<FontData>`, enabling cheap `font_variant()` construction
//! without re-parsing. All table types are `Clone`.

use std::cell::RefCell;
use std::sync::Arc;

use crate::hinting::HintingEngine;
use crate::parser::cmap::CmapTable;
use crate::parser::head::HeadTable;
use crate::parser::hhea::HheaTable;
use crate::parser::hmtx::HmtxTable;
use crate::parser::maxp::MaxpTable;
use crate::parser::name::NameTable;
use crate::parser::os2::Os2Table;

/// All parsed font tables, shared behind `Arc` for cheap `font_variant`.
#[derive(Debug, Clone)]
pub(crate) struct FontData {
    /// Character-to-glyph index mapping.
    pub cmap: CmapTable,
    /// Font header: units_per_em, flags, index_to_loc_format.
    pub head: HeadTable,
    /// Horizontal header: ascent, descent, num_hmetrics.
    pub hhea: HheaTable,
    /// Horizontal metrics: advance_width, lsb per glyph.
    pub hmtx: HmtxTable,
    /// Maximum profile: num_glyphs.
    pub maxp: MaxpTable,
    /// Naming table: family, subfamily.
    pub name: NameTable,
    /// OS/2 metrics: sTypoAscender, sTypoDescender (optional table).
    pub os2: Option<Os2Table>,
    /// Raw 'loca' table data — glyph offsets into glyf.
    pub loca_data: Vec<u8>,
    /// Raw 'glyf' table data — glyph outline bytes.
    pub glyf_data: Vec<u8>,
    /// Format of loca table: 0=short, 1=long (from head.index_to_loc_format).
    pub loca_format: i16,
    /// Requested point size.
    pub size_pt: f32,
    /// Control Value Table (parsed F26Dot6 entries).
    pub cvt: Vec<i32>,
    /// Raw Font Program bytecode.
    pub fpgm: Vec<u8>,
    /// Raw CVT Program bytecode.
    pub prep: Vec<u8>,
    /// Number of CVT entries.
    pub cvt_size: u16,
}

/// A loaded font with shared tables and a point size.
///
/// Clone is O(1) — it increments the `Arc` refcount without re-parsing.
#[derive(Debug, Clone)]
pub struct Font {
    /// Shared parsed font data. All `Font` instances from the same bytes share this.
    pub(crate) data: Arc<FontData>,
    /// Requested point size.
    pub(crate) size_pt: f32,
    /// Optional TrueType hinting engine (present when fpgm or prep tables exist).
    pub hint_engine: Option<RefCell<HintingEngine>>,
}
