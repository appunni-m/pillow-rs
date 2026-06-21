//! 'maxp' table — maximum profile.
//!
//! Contains memory requirements: number of glyphs, max points, max contours.

use crate::error::FontError;

/// Parsed 'maxp' table.
#[derive(Debug, Clone)]
pub(crate) struct MaxpTable {
    /// Total number of glyphs in the font (including glyph 0 /.notdef).
    pub num_glyphs: u16,
}

/// Parse the 'maxp' table from raw bytes. Version 1.0 required (32 bytes).
pub(crate) fn parse_maxp(data: &[u8]) -> Result<MaxpTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "maxp table too short (need 6 bytes)".into(),
        ));
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if version != 0x00010000 {
        return Err(FontError::InvalidFont(format!(
            "maxp: unsupported version 0x{:08X}",
            version
        )));
    }
    let num_glyphs = u16::from_be_bytes([data[4], data[5]]);
    Ok(MaxpTable { num_glyphs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_maxp_with_valid_data() {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // version 1.0
        data[4..6].copy_from_slice(&[0x01, 0xF4]); // num_glyphs = 500

        let maxp = parse_maxp(&data).expect("should parse");
        assert_eq!(maxp.num_glyphs, 500);
    }

    #[test]
    fn wrong_version_is_error() {
        let data = vec![0u8; 6]; // version = 0.0
        let result = parse_maxp(&data);
        assert!(result.is_err());
    }
}
