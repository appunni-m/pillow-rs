//! 'post' table -- PostScript information.
//!
//! Contains underline position, underline thickness, italic angle, and
//! optionally a glyph-name index. We only extract the core underline metrics;
//! the glyph-name index (version 2.0) is skipped.

/// Parsed 'post' table.
#[derive(Debug, Clone)]
pub(crate) struct PostTable {
    /// Underline position in font design units (negative below baseline).
    pub underline_position: i16,
    /// Underline thickness in font design units.
    pub underline_thickness: i16,
}

/// Parse the 'post' table from raw bytes.
///
/// At minimum we need the 32-byte fixed header (version, italicAngle,
/// underlinePosition, underlineThickness, isFixedPitch).
pub(crate) fn parse_post(data: &[u8]) -> Option<PostTable> {
    if data.len() < 32 {
        return None;
    }
    // Version 1.0 (0x00010000), 2.0 (0x00020000), or 3.0 (0x00030000) --
    // all share the same fixed header layout for the first 32 bytes.
    let _version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let _italic_angle = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let underline_position = i16::from_be_bytes([data[8], data[9]]);
    let underline_thickness = i16::from_be_bytes([data[10], data[11]]);
    let _is_fixed_pitch = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

    // Bytes 16-31: minMemType42, maxMemType42, minMemType1, maxMemType1 (unused).

    Some(PostTable {
        underline_position,
        underline_thickness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_post_v1() {
        let mut data = vec![0u8; 32];
        // Version 1.0
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]);
        // italicAngle = 0
        data[4..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // underlinePosition = -100
        data[8..10].copy_from_slice(&[0xFF, 0x9C]);
        // underlineThickness = 50
        data[10..12].copy_from_slice(&[0x00, 0x32]);

        let post = parse_post(&data).expect("should parse");
        assert_eq!(post.underline_position, -100);
        assert_eq!(post.underline_thickness, 50);
    }

    #[test]
    fn short_data_returns_none() {
        let data = vec![0u8; 10];
        assert!(parse_post(&data).is_none());
    }
}
