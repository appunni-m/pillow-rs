//! 'OS/2' table — OS/2 and Windows Metrics. Mirrors `tt_face_load_os2`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_OS2` in `tttables.h`.

/// Parsed 'OS/2' table (the fields reachable from the metrics path).
#[derive(Debug, Clone)]
pub struct Os2Table {
    /// OS/2 table version.
    pub version: u16,
    /// Average weighted escapement.
    pub x_avg_char_width: i16,
    /// Weight class.
    pub us_weight_class: u16,
    /// Width class.
    pub us_width_class: u16,
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
    /// Embedding permission flags (`fsType`).
    pub fs_type: u16,
    /// Subscript horizontal size.
    pub y_subscript_x_size: i16,
    /// Subscript vertical size.
    pub y_subscript_y_size: i16,
    /// Subscript horizontal offset.
    pub y_subscript_x_offset: i16,
    /// Subscript vertical offset.
    pub y_subscript_y_offset: i16,
    /// Superscript horizontal size.
    pub y_superscript_x_size: i16,
    /// Superscript vertical size.
    pub y_superscript_y_size: i16,
    /// Superscript horizontal offset.
    pub y_superscript_x_offset: i16,
    /// Superscript vertical offset.
    pub y_superscript_y_offset: i16,
    /// Strikeout size.
    pub y_strikeout_size: i16,
    /// Strikeout position.
    pub y_strikeout_position: i16,
    /// Family class.
    pub s_family_class: i16,
    /// PANOSE classification bytes.
    pub panose: [u8; 10],
    /// fsSelection flags (byte 62-63).
    fs_selection: u16,
    /// ulUnicodeRange1 (bytes 42-45), bits 0-31 of Unicode character ranges.
    pub ul_unicode_range1: u32,
    /// ulUnicodeRange2 (bytes 46-49), bits 32-63.
    pub ul_unicode_range2: u32,
    /// ulUnicodeRange3 (bytes 50-53), bits 64-95.
    pub ul_unicode_range3: u32,
    /// ulUnicodeRange4 (bytes 54-57), bits 96-127.
    pub ul_unicode_range4: u32,
    /// Vendor identifier.
    pub ach_vend_id: [u8; 4],
    /// First character index.
    pub us_first_char_index: u16,
    /// Last character index.
    pub us_last_char_index: u16,
    /// Code page range 1. Present in OS/2 v1+; zero for older/truncated tables.
    pub ul_code_page_range1: u32,
    /// Code page range 2. Present in OS/2 v1+; zero for older/truncated tables.
    pub ul_code_page_range2: u32,
    /// x-height. Present in OS/2 v2+; zero for older/truncated tables.
    pub sx_height: i16,
    /// Cap height. Present in OS/2 v2+; zero for older/truncated tables.
    pub s_cap_height: i16,
    /// Default character. Present in OS/2 v2+; zero for older/truncated tables.
    pub us_default_char: u16,
    /// Break character. Present in OS/2 v2+; zero for older/truncated tables.
    pub us_break_char: u16,
    /// Maximum context. Present in OS/2 v2+; zero for older/truncated tables.
    pub us_max_context: u16,
    /// Lower optical point size. Present in OS/2 v5+; zero for older/truncated tables.
    pub us_lower_optical_point_size: u16,
    /// Upper optical point size. Present in OS/2 v5+; zero for older/truncated tables.
    pub us_upper_optical_point_size: u16,
}

/// Unicode range bits (ulUnicodeRange1).
/// Bit 7 = Greek and Coptic.
pub const UNICODE_RANGE_GREEK: u32 = 1 << 7;

impl Os2Table {
    /// True when `fsSelection` bit 7 (USE_TYPO_METRICS) is set, matching
    /// FreeType's ascender selection in `sfnt_init_face`.
    pub fn use_typo_metrics(&self) -> bool {
        self.fs_selection & 128 != 0
    }

    /// True when `fsSelection` bit 8 marks this as a WWS-only face.
    pub fn is_wws_only(&self) -> bool {
        self.version != 0xFFFF && self.fs_selection & 256 != 0
    }

    /// Raw `fsSelection` flags as exposed by the public `TT_OS2` record.
    pub fn fs_selection(&self) -> u16 {
        self.fs_selection
    }
}

/// Parse the 'OS/2' table (minimum 78 bytes for the fields we use).
pub fn parse_os2(data: &[u8]) -> Option<Os2Table> {
    if data.len() < 78 {
        return None;
    }
    let mut panose = [0; 10];
    panose.copy_from_slice(&data[32..42]);
    let mut ach_vend_id = [0; 4];
    ach_vend_id.copy_from_slice(&data[58..62]);
    Some(Os2Table {
        version: u16_at(data, 0),
        x_avg_char_width: i16_at(data, 2),
        us_weight_class: u16_at(data, 4),
        us_width_class: u16_at(data, 6),
        s_typo_ascender: i16::from_be_bytes([data[68], data[69]]),
        s_typo_descender: i16::from_be_bytes([data[70], data[71]]),
        s_typo_line_gap: i16::from_be_bytes([data[72], data[73]]),
        us_win_ascent: u16::from_be_bytes([data[74], data[75]]),
        us_win_descent: u16::from_be_bytes([data[76], data[77]]),
        fs_type: u16::from_be_bytes([data[8], data[9]]),
        y_subscript_x_size: i16_at(data, 10),
        y_subscript_y_size: i16_at(data, 12),
        y_subscript_x_offset: i16_at(data, 14),
        y_subscript_y_offset: i16_at(data, 16),
        y_superscript_x_size: i16_at(data, 18),
        y_superscript_y_size: i16_at(data, 20),
        y_superscript_x_offset: i16_at(data, 22),
        y_superscript_y_offset: i16_at(data, 24),
        y_strikeout_size: i16_at(data, 26),
        y_strikeout_position: i16_at(data, 28),
        s_family_class: i16_at(data, 30),
        panose,
        fs_selection: u16_at(data, 62),
        ul_unicode_range1: u32_at(data, 42),
        ul_unicode_range2: u32_at(data, 46),
        ul_unicode_range3: u32_at(data, 50),
        ul_unicode_range4: u32_at(data, 54),
        ach_vend_id,
        us_first_char_index: u16_at(data, 64),
        us_last_char_index: u16_at(data, 66),
        ul_code_page_range1: optional_u32_at(data, 78),
        ul_code_page_range2: optional_u32_at(data, 82),
        sx_height: optional_i16_at(data, 86),
        s_cap_height: optional_i16_at(data, 88),
        us_default_char: optional_u16_at(data, 90),
        us_break_char: optional_u16_at(data, 92),
        us_max_context: optional_u16_at(data, 94),
        us_lower_optical_point_size: optional_u16_at(data, 96),
        us_upper_optical_point_size: optional_u16_at(data, 98),
    })
}

fn i16_at(data: &[u8], offset: usize) -> i16 {
    i16::from_be_bytes([data[offset], data[offset + 1]])
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn optional_i16_at(data: &[u8], offset: usize) -> i16 {
    data.get(offset..offset + 2)
        .map_or(0, |bytes| i16::from_be_bytes([bytes[0], bytes[1]]))
}

fn optional_u16_at(data: &[u8], offset: usize) -> u16 {
    data.get(offset..offset + 2)
        .map_or(0, |bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn optional_u32_at(data: &[u8], offset: usize) -> u32 {
    data.get(offset..offset + 4).map_or(0, |bytes| {
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    })
}
