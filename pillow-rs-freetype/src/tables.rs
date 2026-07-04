//! Parsed font tables: holds all TrueType table data for glyph rendering.
//!
//! [`FontData`] is constructed by [`crate::font::Font::truetype`] and
//! holds the parsed results of all required TrueType tables.

use crate::tt::cmap::CmapTable;
use crate::tt::head::HeadTable;
use crate::tt::hhea::HheaTable;
use crate::tt::hmtx::HmtxTable;
use crate::tt::maxp::MaxpTable;
use crate::tt::name::NameTable;
use crate::tt::os2::Os2Table;
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
    pub vhea: Option<VheaTable>,
    pub vmtx: Option<VmtxTable>,
    pub loca_data: Vec<u8>,
    pub glyf_data: Vec<u8>,
    pub size_pt: f32,
    /// Font program bytecode (fpgm table). Optional — not all fonts have bytecode.
    pub fpgm: Option<Vec<u8>>,
    /// CVT program bytecode (prep table). Optional.
    pub prep: Option<Vec<u8>>,
    /// Control Value Table (cvt table) in 26.6 format. Optional.
    pub cvt: Option<Vec<i32>>,
}
