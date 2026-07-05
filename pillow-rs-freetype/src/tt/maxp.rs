//! 'maxp' table — maximum profile. Mirrors `tt_load_maxp`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_MaxProfile` in `tttables.h`.

use crate::error::FontError;

/// Parsed 'maxp' table.
#[derive(Debug, Clone)]
pub struct MaxpTable {
    /// Total number of glyphs (including glyph 0 / .notdef).
    pub num_glyphs: u16,
    /// Maximum points in a simple glyph (used for buffer sizing).
    pub max_points: u16,
    /// Maximum contours in a simple glyph.
    pub max_contours: u16,
    /// Number of twilight-zone points available to TrueType bytecode.
    pub max_twilight_points: u16,
    /// Number of storage area locations available to TrueType bytecode.
    pub max_storage: u16,
    /// Maximum component depth for composite glyphs.
    pub max_component_depth: u16,
}

/// Parse the 'maxp' table. Requires version 1.0 (32 bytes) for TrueType.
pub fn parse_maxp(data: &[u8]) -> Result<MaxpTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "maxp table too short (need 6 bytes)".into(),
        ));
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if version != 0x0001_0000 {
        return Err(FontError::InvalidFont(format!(
            "maxp: unsupported version 0x{:08X}",
            version
        )));
    }
    let num_glyphs = u16::from_be_bytes([data[4], data[5]]);
    let max_points = if data.len() >= 8 {
        u16::from_be_bytes([data[6], data[7]])
    } else {
        0
    };
    let max_contours = if data.len() >= 10 {
        u16::from_be_bytes([data[8], data[9]])
    } else {
        0
    };
    let max_twilight_points = if data.len() >= 18 {
        u16::from_be_bytes([data[16], data[17]])
    } else {
        0
    };
    let max_storage = if data.len() >= 20 {
        u16::from_be_bytes([data[18], data[19]])
    } else {
        0
    };
    let max_component_depth = if data.len() >= 32 {
        u16::from_be_bytes([data[30], data[31]])
    } else {
        0
    };
    Ok(MaxpTable {
        num_glyphs,
        max_points,
        max_contours,
        max_twilight_points,
        max_storage,
        max_component_depth,
    })
}
