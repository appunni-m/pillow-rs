//! 'OS/2' table — OS/2 and Windows Metrics. Mirrors `tt_face_load_os2`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_OS2` in `tttables.h`.

/// Parsed 'OS/2' table (the fields reachable from the metrics path).
#[derive(Debug, Clone)]
pub struct Os2Table {
    /// Typographic ascender (font units).
    pub s_typo_ascender: i16,
    /// Typographic descender (font units).
    pub s_typo_descender: i16,
    /// Typographic line gap.
    pub s_typo_line_gap: i16,
    /// Windows ascender (font units).
    pub us_win_ascent: u16,
    /// Windows descender (font units, positive = down).
    pub us_win_descent: u16,
}

/// Parse the 'OS/2' table (minimum 78 bytes for the fields we use).
pub fn parse_os2(data: &[u8]) -> Option<Os2Table> {
    if data.len() < 78 {
        return None;
    }
    Some(Os2Table {
        s_typo_ascender: i16::from_be_bytes([data[68], data[69]]),
        s_typo_descender: i16::from_be_bytes([data[70], data[71]]),
        s_typo_line_gap: i16::from_be_bytes([data[72], data[73]]),
        us_win_ascent: u16::from_be_bytes([data[74], data[75]]),
        us_win_descent: u16::from_be_bytes([data[76], data[77]]),
    })
}
