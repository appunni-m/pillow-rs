//! 'maxp' table — maximum profile. Mirrors `tt_load_maxp`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_MaxProfile` in `tttables.h`.

use crate::error::FontError;

/// Parsed 'maxp' table.
#[derive(Debug, Clone, Default)]
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

/// Parse the `maxp` table.
pub fn parse_maxp(data: &[u8]) -> Result<MaxpTable, FontError> {
    if data.len() < 6 {
        // sfnt_load_face ignores tt_face_load_maxp errors and continues with
        // its zero-initialized profile.
        return Ok(MaxpTable::default());
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let num_glyphs = u16::from_be_bytes([data[4], data[5]]);

    // FreeType's tt_face_load_maxp reads only the six-byte header below
    // version 1.0 and a complete 26-byte extra frame otherwise.
    let (max_points, max_contours, max_twilight_points, max_storage, max_component_depth) =
        if version >= 0x0001_0000 {
            if data.len() < 32 {
                return Err(FontError::InvalidFont(
                    "maxp version 1 table too short (need 32 bytes)".into(),
                ));
            }
            (
                u16::from_be_bytes([data[6], data[7]]),
                u16::from_be_bytes([data[8], data[9]]),
                u16::from_be_bytes([data[16], data[17]]),
                u16::from_be_bytes([data[18], data[19]]),
                u16::from_be_bytes([data[30], data[31]]),
            )
        } else {
            (0, 0, 0, 0, 0)
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
