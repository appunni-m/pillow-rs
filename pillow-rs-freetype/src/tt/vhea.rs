//! 'vhea' table — Vertical Header.
//!
//! Reference: `TT_VertHeader` in FreeType's `tttables.h`.

use crate::error::FontError;

/// Parsed 'vhea' table.
#[derive(Debug, Clone)]
pub struct VheaTable {
    /// Maximum vertical advance height in font units.
    pub advance_height_max: u16,
    /// Number of vmtx entries with explicit advance heights.
    pub num_vmetrics: u16,
}

/// Parse the 'vhea' table (36 bytes).
pub fn parse_vhea(data: &[u8]) -> Result<VheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "vhea table too short (need 36 bytes)".into(),
        ));
    }
    Ok(VheaTable {
        advance_height_max: u16::from_be_bytes([data[10], data[11]]),
        num_vmetrics: u16::from_be_bytes([data[34], data[35]]),
    })
}
