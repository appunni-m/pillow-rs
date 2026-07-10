//! 'post' table scalar metadata.
//!
//! Reference: `TT_Postscript` in FreeType's public TrueType table structs.

/// Parsed 'post' table fields used by face metadata.
#[derive(Debug, Clone)]
pub struct PostTable {
    /// PostScript table format in 16.16 fixed-point form.
    pub format_type: u32,
    /// Underline position in font units.
    pub underline_position: i16,
    /// Underline thickness in font units.
    pub underline_thickness: i16,
    /// Non-zero if the face reports fixed-pitch advances.
    pub is_fixed_pitch: u32,
}

/// Parse the 'post' table header fields used by `FT_FaceRec`.
pub fn parse_post(data: &[u8]) -> Option<PostTable> {
    if data.len() < 16 {
        return None;
    }

    Some(PostTable {
        format_type: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
        underline_position: i16::from_be_bytes([data[8], data[9]]),
        underline_thickness: i16::from_be_bytes([data[10], data[11]]),
        is_fixed_pitch: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
    })
}
