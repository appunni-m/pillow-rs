//! 'OS/2' table — OS/2 and Windows Metrics.
//!
//! Contains typographic metrics used by getmetrics(): sTypoAscender,
//! sTypoDescender, sTypoLineGap, and Windows-specific usWinAscent/usWinDescent.

/// Parsed 'OS/2' table (version 0+, first 40 bytes).
#[derive(Debug, Clone)]
pub(crate) struct Os2Table {
    /// Typographic ascender (font units, positive up).
    pub s_typo_ascender: i16,
    /// Typographic descender (font units, negative down).
    pub s_typo_descender: i16,
    /// Typographic line gap.
    pub s_typo_line_gap: i16,
    /// Windows ascender (font units).
    pub us_win_ascent: u16,
    /// Windows descender (font units, positive value meaning down).
    pub us_win_descent: u16,
}

/// Parse 'OS/2' table from raw bytes (minimum 68 bytes for version 0).
pub(crate) fn parse_os2(data: &[u8]) -> Option<Os2Table> {
    if data.len() < 68 {
        return None;
    }
    let _version = u16::from_be_bytes([data[0], data[1]]);
    let _x_avg_char_width = i16::from_be_bytes([data[2], data[3]]);
    let _us_weight_class = u16::from_be_bytes([data[4], data[5]]);
    let _us_width_class = u16::from_be_bytes([data[6], data[7]]);

    // sTypoAscender at offset 68, sTypoDescender at 70, sTypoLineGap at 72
    let typo_off = 68usize;
    if data.len() < typo_off + 6 {
        return None;
    }
    let s_typo_ascender = i16::from_be_bytes([data[typo_off], data[typo_off + 1]]);
    let s_typo_descender = i16::from_be_bytes([data[typo_off + 2], data[typo_off + 3]]);
    let s_typo_line_gap = i16::from_be_bytes([data[typo_off + 4], data[typo_off + 5]]);

    // usWinAscent at offset 74, usWinDescent at 76
    let win_off = 74usize;
    if data.len() < win_off + 4 {
        return None;
    }
    let us_win_ascent = u16::from_be_bytes([data[win_off], data[win_off + 1]]);
    let us_win_descent = u16::from_be_bytes([data[win_off + 2], data[win_off + 3]]);

    Some(Os2Table {
        s_typo_ascender,
        s_typo_descender,
        s_typo_line_gap,
        us_win_ascent,
        us_win_descent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_os2_table() {
        let mut data = vec![0u8; 78];
        data[68..70].copy_from_slice(&[0x06, 0x00]); // sTypoAscender = 1536
        data[70..72].copy_from_slice(&[0xFE, 0x00]); // sTypoDescender = -512
        data[72..74].copy_from_slice(&[0x00, 0x00]); // sTypoLineGap = 0
        data[74..76].copy_from_slice(&[0x07, 0x00]); // usWinAscent = 1792
        data[76..78].copy_from_slice(&[0x02, 0x00]); // usWinDescent = 512

        let os2 = parse_os2(&data).expect("should parse");
        assert_eq!(os2.s_typo_ascender, 1536);
        assert_eq!(os2.s_typo_descender, -512);
        assert_eq!(os2.us_win_ascent, 1792);
        assert_eq!(os2.us_win_descent, 512);
    }
}
