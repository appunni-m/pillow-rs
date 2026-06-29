//! 'head' table — font header. Mirrors `tt_load_head` field offsets.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_Header` in `include/freetype/tttables.h`.

use crate::error::FontError;

/// Parsed 'head' table (the fields reachable from the rendering path).
#[derive(Debug, Clone)]
pub struct HeadTable {
    /// Font design units per em-square (typically 1000 or 2048).
    pub units_per_em: u16,
    /// Format of the 'loca' table: 0 = short, 1 = long.
    pub index_to_loc_format: i16,
    /// Font flags (bit 0 baseline-at-y0, etc.).
    pub flags: u16,
    /// Macintosh style flags (bit 0=bold, bit 1=italic).
    pub mac_style: u16,
    /// Lowest recPPEM (smallest size the font is designed for).
    pub lowest_rec_ppem: u16,
}

/// Parse the 'head' table from raw bytes (54 bytes minimum).
pub fn parse_head(data: &[u8]) -> Result<HeadTable, FontError> {
    if data.len() < 54 {
        return Err(FontError::InvalidFont(
            "head table too short (need 54 bytes)".into(),
        ));
    }
    let units_per_em = u16::from_be_bytes([data[18], data[19]]);
    let flags = u16::from_be_bytes([data[16], data[17]]);
    let mac_style = u16::from_be_bytes([data[44], data[45]]);
    let lowest_rec_ppem = u16::from_be_bytes([data[46], data[47]]);
    let index_to_loc_format = i16::from_be_bytes([data[50], data[51]]);

    if units_per_em == 0 {
        return Err(FontError::InvalidFont("head: units_per_em is zero".into()));
    }

    Ok(HeadTable {
        units_per_em,
        index_to_loc_format,
        flags,
        mac_style,
        lowest_rec_ppem,
    })
}
