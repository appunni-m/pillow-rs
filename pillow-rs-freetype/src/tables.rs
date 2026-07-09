//! Parsed font tables: holds all TrueType table data for glyph rendering.
//!
//! [`FontData`] is constructed by [`crate::font::Font::truetype`] and
//! holds the parsed results of all required TrueType tables.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::tt::cmap::CmapTable;
use crate::tt::hdmx::HdmxTable;
use crate::tt::head::HeadTable;
use crate::tt::hhea::HheaTable;
use crate::tt::hmtx::HmtxTable;
use crate::tt::kern::KernTable;
use crate::tt::maxp::MaxpTable;
use crate::tt::name::NameTable;
use crate::tt::os2::Os2Table;
use crate::tt::post::PostTable;
use crate::tt::vhea::VheaTable;
use crate::tt::vmtx::VmtxTable;

/// All parsed font tables for one face, plus the requested point size.
#[derive(Debug, Clone)]
pub struct FontData {
    pub raw_data: Vec<u8>,
    pub face_offset: usize,
    pub face_index: usize,
    pub num_faces: usize,
    pub table_directory: crate::tt::TableDirectory,
    pub cmap: CmapTable,
    pub head: HeadTable,
    pub hhea: HheaTable,
    pub hmtx: HmtxTable,
    pub maxp: MaxpTable,
    pub name: NameTable,
    pub os2: Option<Os2Table>,
    pub post: Option<PostTable>,
    pub vhea: Option<VheaTable>,
    pub vmtx: Option<VmtxTable>,
    pub hdmx: Option<HdmxTable>,
    pub kern: Option<KernTable>,
    pub loca_data: Vec<u8>,
    pub glyf_data: Vec<u8>,
    pub size_pt: Cell<f32>,
    /// Font program bytecode (fpgm table). Optional — not all fonts have bytecode.
    pub fpgm: Option<Vec<u8>>,
    /// CVT program bytecode (prep table). Optional.
    pub prep: Option<Vec<u8>>,
    /// Control Value Table (cvt table) in 26.6 format. Optional.
    pub cvt: Option<Vec<i32>>,
    /// Cached parsed glyph outlines.  Populated lazily during glyph loads
    /// to avoid re-parsing the glyf/loca table on every call.
    pub glyph_cache: RefCell<HashMap<u16, crate::tt::glyf::GlyphOutline>>,
}

impl FontData {
    /// Load a glyph outline, returning a clone from the cache on hit.
    /// The cache avoids re-parsing the raw glyf/loca table on every glyph load.
    pub fn load_glyph_outline(
        &self,
        glyph_index: u16,
    ) -> Result<crate::tt::glyf::GlyphOutline, crate::error::FontError> {
        {
            let cache = self.glyph_cache.borrow();
            if let Some(outline) = cache.get(&glyph_index) {
                return Ok(outline.clone());
            }
        }
        let outline = crate::tt::glyf::load_glyph(
            &self.glyf_data,
            &self.loca_data,
            self.head.index_to_loc_format,
            glyph_index,
            &self.hmtx,
        )?;
        self.glyph_cache
            .borrow_mut()
            .insert(glyph_index, outline.clone());
        Ok(outline)
    }
}
