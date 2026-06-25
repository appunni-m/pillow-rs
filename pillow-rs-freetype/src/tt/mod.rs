//! SFNT / TrueType table parsing.
//!
//! Faithful ports of the table-loading paths in FreeType's
//! `src/sfnt/ttload.c`, `src/sfnt/ttcmap.c`, and `src/truetype/ttgload.c`
//! for the subset needed by PIL rendering (head, maxp, hhea, hmtx, cmap,
//! name, loca, glyf, OS/2).

pub mod cmap;
pub mod glyf;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod loca;
pub mod maxp;
pub mod name;
pub mod os2;

use crate::error::FontError;

/// Magic bytes identifying an OpenType font with CFF outlines.
pub const OTTO_MAGIC: u32 = 0x4F54_544F; // "OTTO"
/// Magic bytes identifying a TrueType font.
pub const TRUE_MAGIC: u32 = 0x0001_0000;

/// A reference to a single font table within the raw data.
#[derive(Debug, Clone, Copy)]
pub struct TableRecord {
    /// 4-byte table tag (e.g. `b"cmap"`).
    pub tag: u32,
    /// Byte offset from start of font data.
    pub offset: u32,
    /// Length in bytes.
    pub length: u32,
}

/// Parsed table directory — maps table tags to their data slices.
#[derive(Debug)]
pub struct TableDirectory {
    /// Individual table records, in order of appearance.
    pub records: Vec<TableRecord>,
}

impl TableDirectory {
    /// Look up a table by its 4-byte tag, returning a slice into the font data.
    pub fn find<'a>(&self, data: &'a [u8], tag: u32) -> Option<&'a [u8]> {
        for record in &self.records {
            if record.tag == tag {
                let start = record.offset as usize;
                let end = start + record.length as usize;
                return data.get(start..end);
            }
        }
        None
    }
}

/// Parse the TrueType/OpenType offset table + table directory.
///
/// Mirrors `tt_face_load_font_dir` in `src/sfnt/ttload.c`: read the sfVersion,
/// then `numTables` directory records of 16 bytes each.
pub fn parse_table_directory(data: &[u8]) -> Result<TableDirectory, FontError> {
    if data.len() < 12 {
        return Err(FontError::InvalidFont(
            "data too short for offset table (need 12 bytes)".into(),
        ));
    }

    let sf_version = read_u32(data, 0);
    if sf_version != TRUE_MAGIC && sf_version != OTTO_MAGIC {
        return Err(FontError::InvalidFont(format!(
            "unknown sfVersion: 0x{:08X}",
            sf_version
        )));
    }

    let num_tables = read_u16(data, 4) as usize;
    let dir_start = 12usize;
    let dir_end = dir_start + num_tables * 16;
    if data.len() < dir_end {
        return Err(FontError::InvalidFont(format!(
            "data too short for {num_tables} table records"
        )));
    }

    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let off = dir_start + i * 16;
        records.push(TableRecord {
            tag: read_u32(data, off),
            offset: read_u32(data, off + 8),
            length: read_u32(data, off + 12),
        });
    }

    Ok(TableDirectory { records })
}

/// Build a u32 tag from 4 ASCII bytes. E.g. `tag(b"cmap")` = `0x636D6170`.
#[inline]
pub const fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

// ── big-endian primitives ──────────────────────────────────────────────────

#[inline]
pub(crate) fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub(crate) fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_be_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub(crate) fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}
