//! 'head' table — font header.
//!
//! Contains global font information: units_per_em, glyph data format,
//! font direction hints, and creation/modification timestamps.

use crate::error::FontError;

/// Parsed 'head' table.
#[derive(Debug, Clone)]
pub(crate) struct HeadTable {
    /// Font design units per em-square. Typically 1000 or 2048.
    pub units_per_em: u16,
    /// Format of the 'loca' table: 0 = short (offset/2), 1 = long (direct).
    pub index_to_loc_format: i16,
    /// Font flags (bit 3 = instructions may depend on point size).
    pub flags: u16,
}

/// Parse the 'head' table from raw bytes.
pub(crate) fn parse_head(data: &[u8]) -> Result<HeadTable, FontError> {
    if data.len() < 54 {
        return Err(FontError::InvalidFont(
            "head table too short (need 54 bytes)".into(),
        ));
    }
    let units_per_em = u16::from_be_bytes([data[18], data[19]]);
    let index_to_loc_format = i16::from_be_bytes([data[50], data[51]]);
    let flags = u16::from_be_bytes([data[16], data[17]]);

    if units_per_em == 0 {
        return Err(FontError::InvalidFont("head: units_per_em is zero".into()));
    }

    Ok(HeadTable {
        units_per_em,
        index_to_loc_format,
        flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_head_table() {
        let mut data = vec![0u8; 54];
        data[18..20].copy_from_slice(&[0x08, 0x00]); // units_per_em = 2048
        data[50..52].copy_from_slice(&[0x00, 0x01]); // index_to_loc_format = 1 (long)
        data[16..18].copy_from_slice(&[0x00, 0x08]); // flags = 8 (bit 3 set)

        let head = parse_head(&data).expect("should parse");
        assert_eq!(head.units_per_em, 2048);
        assert_eq!(head.index_to_loc_format, 1);
        assert_eq!(head.flags, 8);
    }

    #[test]
    fn zero_units_per_em_is_error() {
        let data = vec![0u8; 54];
        let result = parse_head(&data);
        assert!(result.is_err());
    }
}
