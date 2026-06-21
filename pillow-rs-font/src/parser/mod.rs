//! TrueType/OpenType table directory parsing.
//!
//! Reads the offset table and table directory from raw font bytes.
//! Matches FreeType 2.6's SFNT table loading in `sfnt/ttload.c`.

use crate::error::FontError;

/// Magic bytes identifying an OpenType font with TrueType outlines.
const OTTO_MAGIC: u32 = 0x4F54544F; // "OTTO"
/// Magic bytes identifying a TrueType font.
const TRUE_MAGIC: u32 = 0x00010000;

/// A reference to a single font table within the raw data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableRecord {
    /// 4-byte table tag (e.g. b'cmap', b'head').
    pub tag: u32,
    /// Byte offset from start of font data.
    pub offset: u32,
    /// Length in bytes.
    pub length: u32,
}

/// Parsed table directory — maps table tags to their data slices.
pub(crate) struct TableDirectory {
    /// Number of tables in the directory.
    pub num_tables: u16,
    /// Individual table records, in order of appearance.
    pub records: Vec<TableRecord>,
}

/// Read a big-endian u16 from a byte slice at the given offset.
#[inline]
fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let b = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

/// Read a big-endian u32 from a byte slice at the given offset.
#[inline]
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parse the TrueType/OpenType table directory from raw font bytes.
///
/// Returns the table directory if the data is a valid font.
pub(crate) fn parse_table_directory(data: &[u8]) -> Result<TableDirectory, FontError> {
    if data.len() < 12 {
        return Err(FontError::InvalidFont(
            "data too short for offset table (need 12 bytes)".into(),
        ));
    }

    let sf_version =
        read_u32(data, 0).ok_or_else(|| FontError::InvalidFont("cannot read sfVersion".into()))?;

    // Accept TrueType (0x00010000) and OpenType with TrueType outlines ("OTTO")
    if sf_version != TRUE_MAGIC && sf_version != OTTO_MAGIC {
        return Err(FontError::InvalidFont(format!(
            "unknown sfVersion: 0x{:08X}",
            sf_version
        )));
    }

    let num_tables =
        read_u16(data, 4).ok_or_else(|| FontError::InvalidFont("cannot read numTables".into()))?;

    let entry_size = 16usize;
    let dir_start = 12usize;
    let dir_end = dir_start + (num_tables as usize) * entry_size;

    if data.len() < dir_end {
        return Err(FontError::InvalidFont(format!(
            "data too short for {} table records",
            num_tables
        )));
    }

    let mut records = Vec::with_capacity(num_tables as usize);
    for i in 0..num_tables as usize {
        let off = dir_start + i * entry_size;
        let tag = read_u32(data, off)
            .ok_or_else(|| FontError::InvalidFont("cannot read table tag".into()))?;
        let _checksum = read_u32(data, off + 4);
        let offset = read_u32(data, off + 8)
            .ok_or_else(|| FontError::InvalidFont("cannot read table offset".into()))?;
        let length = read_u32(data, off + 12)
            .ok_or_else(|| FontError::InvalidFont("cannot read table length".into()))?;

        records.push(TableRecord {
            tag,
            offset,
            length,
        });
    }

    Ok(TableDirectory {
        num_tables,
        records,
    })
}

/// Look up a table by its 4-byte tag, returning a slice into the font data.
pub(crate) fn find_table<'a>(data: &'a [u8], dir: &TableDirectory, tag: u32) -> Option<&'a [u8]> {
    for record in &dir.records {
        if record.tag == tag {
            let start = record.offset as usize;
            let end = start + record.length as usize;
            return data.get(start..end);
        }
    }
    None
}

pub(crate) mod cmap;
pub(crate) mod head;
pub(crate) mod hhea;
pub(crate) mod hmtx;
pub(crate) mod kern;
pub(crate) mod loca_glyf;
pub(crate) mod maxp;
pub(crate) mod name;
pub(crate) mod os2;
pub(crate) mod post;

/// Build a u32 tag from 4 ASCII bytes. E.g., tag(b"cmap") = 0x636D6170.
#[inline]
pub(crate) const fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_returns_invalid_font_error() {
        let result = parse_table_directory(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn valid_true_type_magic_parses_directory() {
        // Minimal TrueType font: sfVersion + 1 table
        let mut data = vec![0u8; 12 + 16];
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // TRUE_MAGIC
        data[4..6].copy_from_slice(&[0x00, 0x01]); // numTables = 1
                                                   // table record at offset 12
        data[12..16].copy_from_slice(b"cmap");
        data[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x1C]); // offset = 28

        let dir = parse_table_directory(&data).expect("should parse");
        assert_eq!(dir.num_tables, 1);
        assert_eq!(dir.records[0].tag, tag(b"cmap"));
    }

    #[test]
    fn otto_magic_also_accepted() {
        let mut data = vec![0u8; 12 + 16];
        data[0..4].copy_from_slice(b"OTTO");
        data[4..6].copy_from_slice(&[0x00, 0x01]);
        data[12..16].copy_from_slice(b"cmap");
        data[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x1C]);

        let dir = parse_table_directory(&data).expect("OTTO should parse");
        assert_eq!(dir.num_tables, 1);
    }
}
