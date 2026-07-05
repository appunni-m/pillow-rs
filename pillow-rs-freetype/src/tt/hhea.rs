//! 'hhea' table — Horizontal Header. Mirrors `tt_load_hhea`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_HoriHeader` in `tttables.h`.

use crate::error::FontError;

/// Parsed 'hhea' table.
#[derive(Debug, Clone)]
pub struct HheaTable {
    /// Typographic ascent (font units, positive up).
    pub ascent: i16,
    /// Typographic descent (font units, negative down).
    pub descent: i16,
    /// Typographic line gap.
    pub line_gap: i16,
    /// Maximum horizontal advance width in font units.
    pub advance_width_max: u16,
    /// Number of hmtx entries with explicit advance widths.
    pub num_hmetrics: u16,
}

/// Parse the 'hhea' table (36 bytes).
pub fn parse_hhea(data: &[u8]) -> Result<HheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "hhea table too short (need 36 bytes)".into(),
        ));
    }
    Ok(HheaTable {
        ascent: i16::from_be_bytes([data[4], data[5]]),
        descent: i16::from_be_bytes([data[6], data[7]]),
        line_gap: i16::from_be_bytes([data[8], data[9]]),
        advance_width_max: u16::from_be_bytes([data[10], data[11]]),
        num_hmetrics: u16::from_be_bytes([data[34], data[35]]),
    })
}
