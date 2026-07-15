//! Parsed font tables: holds all TrueType table data for glyph rendering.
//!
//! [`FontData`] is constructed by [`crate::font::Font::truetype`] and
//! holds the parsed results of all required TrueType tables.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use crate::tt::cff::CffTable;
use crate::tt::cmap::CmapTable;
use crate::tt::fvar::FvarTable;
use crate::tt::gasp::GaspTable;
use crate::tt::hdmx::HdmxTable;
use crate::tt::head::HeadTable;
use crate::tt::hhea::HheaTable;
use crate::tt::hmtx::HmtxTable;
use crate::tt::kern::KernTable;
use crate::tt::maxp::MaxpTable;
use crate::tt::name::NameTable;
use crate::tt::os2::Os2Table;
use crate::tt::post::PostTable;
use crate::tt::sbit::SbitTable;
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
    pub fvar: Option<FvarTable>,
    pub gasp: Option<GaspTable>,
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
    pub sbit: Option<SbitTable>,
    pub cff: Option<CffTable>,
    pub loca_data: Vec<u8>,
    pub glyf_data: Vec<u8>,
    pub size_pt: Cell<f32>,
    pub size_x_scale: Cell<i32>,
    pub size_y_scale: Cell<i32>,
    pub size_tt_scale: Cell<i32>,
    pub size_tt_ppem: Cell<i32>,
    pub size_tt_point_size: Cell<i32>,
    /// Active 2×2 transform set via FT_Set_Transform.  The scaler reads these
    /// before the auto-hinter runs so hinting decisions match the transformed
    /// geometry.  Identity is (0x10000, 0, 0, 0x10000, 0, 0).
    pub transform_xx: Cell<i32>,
    pub transform_xy: Cell<i32>,
    pub transform_yx: Cell<i32>,
    pub transform_yy: Cell<i32>,
    pub transform_dx: Cell<i32>,
    pub transform_dy: Cell<i32>,
    /// Font program bytecode (fpgm table). Optional — not all fonts have bytecode.
    pub fpgm: Option<Vec<u8>>,
    /// CVT program bytecode (prep table). Optional.
    pub prep: Option<Vec<u8>>,
    /// Control Value Table (cvt table) in 26.6 format. Optional.
    pub cvt: Option<Vec<i32>>,
    /// Cached parsed glyph outlines.  Populated lazily during glyph loads
    /// to avoid re-parsing the glyf/loca table on every call.
    pub glyph_cache: RefCell<HashMap<u16, Rc<crate::tt::glyf::GlyphOutline>>>,
    /// Back-pointer to the `Arc<FontData>` that owns this instance.
    /// Set once during font construction; used to avoid expensive clones.
    #[doc(hidden)]
    pub self_arc: OnceLock<Arc<FontData>>,
}

impl FontData {
    /// Load a glyph outline, returning a shared reference on cache hit.
    /// Uses Rc to avoid cloning the entire outline Vec on every access.
    pub fn load_glyph_outline(
        &self,
        glyph_index: u16,
    ) -> Result<Rc<crate::tt::glyf::GlyphOutline>, crate::error::FontError> {
        {
            let cache = self.glyph_cache.borrow();
            if let Some(outline) = cache.get(&glyph_index) {
                return Ok(Rc::clone(outline));
            }
        }
        if let Some(cff) = &self.cff {
            let outline = Rc::new(cff.load_glyph(glyph_index)?);
            self.glyph_cache
                .borrow_mut()
                .insert(glyph_index, Rc::clone(&outline));
            return Ok(outline);
        }
        let outline = Rc::new(crate::tt::glyf::load_glyph(
            &self.glyf_data,
            &self.loca_data,
            self.head.index_to_loc_format,
            glyph_index,
            &self.hmtx,
        )?);
        self.glyph_cache
            .borrow_mut()
            .insert(glyph_index, Rc::clone(&outline));
        Ok(outline)
    }
}
