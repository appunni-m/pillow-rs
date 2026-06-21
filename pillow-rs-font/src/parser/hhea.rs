//! 'hhea' table — Horizontal Header.
//!
//! Contains font-wide horizontal metrics: ascent, descent, line gap,
//! and the number of hmtx entries with explicit advance widths.

use crate::error::FontError;

/// Parsed 'hhea' table.
#[derive(Debug, Clone)]
pub(crate) struct HheaTable {
    /// Typographic ascent (font units, positive up).
    pub ascent: i16,
    /// Typographic descent (font units, negative down).
    pub descent: i16,
    /// Typographic line gap.
    pub line_gap: i16,
    /// Number of hmtx entries that have explicit advance widths.
    /// Remaining glyphs use the last advance width.
    pub num_hmetrics: u16,
}

/// Parse 'hhea' table from raw bytes.
pub(crate) fn parse_hhea(data: &[u8]) -> Result<HheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "hhea table too short (need 36 bytes)".into(),
        ));
    }
    let ascent = i16::from_be_bytes([data[4], data[5]]);
    let descent = i16::from_be_bytes([data[6], data[7]]);
    let line_gap = i16::from_be_bytes([data[8], data[9]]);
    let _advance_width_max = u16::from_be_bytes([data[10], data[11]]);
    // num_hmetrics is at offset 34 (bytes 34-35)
    let num_hmetrics = u16::from_be_bytes([data[34], data[35]]);

    Ok(HheaTable {
        ascent,
        descent,
        line_gap,
        num_hmetrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_hhea() {
        let mut data = vec![0u8; 36];
        data[4..6].copy_from_slice(&[0x07, 0x00]); // ascent = 1792
        data[6..8].copy_from_slice(&[0xFE, 0x00]); // descent = -512
        data[34..36].copy_from_slice(&[0x01, 0xF4]); // num_hmetrics = 500

        let hhea = parse_hhea(&data).expect("should parse");
        assert_eq!(hhea.ascent, 1792);
        assert_eq!(hhea.descent, -512);
        assert_eq!(hhea.num_hmetrics, 500);
    }
}
