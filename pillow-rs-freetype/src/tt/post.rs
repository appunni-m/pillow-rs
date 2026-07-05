//! 'post' table scalar metadata.
//!
//! Reference: `TT_Postscript` in FreeType's public TrueType table structs.

/// Parsed 'post' table fields used by face metadata.
#[derive(Debug, Clone)]
pub struct PostTable {
    /// Underline position in font units.
    pub underline_position: i16,
    /// Underline thickness in font units.
    pub underline_thickness: i16,
}

/// Parse the 'post' table header fields used by `FT_FaceRec`.
pub fn parse_post(data: &[u8]) -> Option<PostTable> {
    if data.len() < 12 {
        return None;
    }

    Some(PostTable {
        underline_position: i16::from_be_bytes([data[8], data[9]]),
        underline_thickness: i16::from_be_bytes([data[10], data[11]]),
    })
}
