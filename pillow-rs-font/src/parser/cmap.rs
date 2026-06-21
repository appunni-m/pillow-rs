//! 'cmap' table — character to glyph index mapping.
//!
//! Supports format 4 (BMP segment mapping) and format 12 (full Unicode).
//! Format selection priority: 12 → 4 (matching FreeType 2.6).

use crate::error::FontError;
use log::warn;

/// A character-to-glyph mapping table.
#[derive(Debug, Clone)]
pub(crate) struct CmapTable {
    /// Format 4 subtables (one per encoding record).
    pub format4: Vec<Format4Subtable>,
    /// Format 12 subtables (one per encoding record).
    pub format12: Vec<Format12Subtable>,
}

/// Format 4: Segment mapping for Unicode BMP (U+0000–U+FFFF).
#[derive(Debug, Clone)]
pub(crate) struct Format4Subtable {
    /// Platform ID from the encoding record (3 = Windows).
    pub platform_id: u16,
    /// Encoding ID from the encoding record (1 = Unicode BMP).
    pub encoding_id: u16,
    /// End character codes for each segment.
    pub end_codes: Vec<u16>,
    /// Start character codes for each segment.
    pub start_codes: Vec<u16>,
    /// Delta values (glyph_index = char_code + delta if not in range_offset).
    pub id_deltas: Vec<i16>,
    /// Range offsets for segments with non-contiguous mappings.
    pub id_range_offsets: Vec<u16>,
    /// Glyph ID array (indexed via range offsets).
    pub glyph_id_array: Vec<u16>,
}

/// Format 12: Segmented coverage for full Unicode (U+0000–U+10FFFF).
#[derive(Debug, Clone)]
pub(crate) struct Format12Subtable {
    /// Platform ID from the encoding record (3 = Windows).
    pub platform_id: u16,
    /// Encoding ID (10 = Unicode full repertoire).
    pub encoding_id: u16,
    /// Start character codes for each group.
    pub start_codes: Vec<u32>,
    /// End character codes for each group.
    pub end_codes: Vec<u32>,
    /// Start glyph IDs for each group (glyph = start_glyph + (char - start)).
    pub start_glyph_ids: Vec<u32>,
}

/// Parsed encoding record from the cmap header.
#[derive(Debug, Clone)]
struct EncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    subtable_offset: u32,
}

impl CmapTable {
    /// Map a Unicode codepoint to a glyph index.
    ///
    /// Returns `None` if no mapping exists (caller should use glyph 0 = .notdef).
    pub fn map(&self, codepoint: u32) -> Option<u16> {
        // Try format 12 first (preferred for full Unicode coverage)
        for sub in &self.format12 {
            if let Some(glyph) = sub.map_codepoint(codepoint) {
                return Some(glyph);
            }
        }
        // Fall back to format 4 (BMP only)
        if codepoint <= 0xFFFF {
            for sub in &self.format4 {
                if let Some(glyph) = sub.map_codepoint(codepoint as u16) {
                    return Some(glyph);
                }
            }
        }
        None
    }
}

impl Format12Subtable {
    fn map_codepoint(&self, codepoint: u32) -> Option<u16> {
        // Binary search through groups
        for i in 0..self.start_codes.len() {
            if codepoint >= self.start_codes[i] && codepoint <= self.end_codes[i] {
                let offset = codepoint - self.start_codes[i];
                return Some((self.start_glyph_ids[i] + offset) as u16);
            }
            if codepoint < self.start_codes[i] {
                break; // groups are sorted — no need to continue
            }
        }
        None
    }
}

impl Format4Subtable {
    fn map_codepoint(&self, char_code: u16) -> Option<u16> {
        // Find the segment containing char_code
        for seg in 0..self.end_codes.len() {
            if char_code > self.end_codes[seg] {
                continue;
            }
            if char_code < self.start_codes[seg] {
                return None; // before first segment
            }
            if self.id_range_offsets[seg] == 0 {
                // Contiguous mapping: glyph = (char + delta) mod 65536
                let glyph =
                    (char_code as u32).wrapping_add_signed(self.id_deltas[seg] as i32) as u16;
                return Some(glyph);
            }
            // Non-contiguous: use range offset to index into glyph_id_array
            let range_off = self.id_range_offsets[seg] as usize;
            let idx_in_seg = (char_code - self.start_codes[seg]) as usize;
            let glyph_idx = (range_off / 2) + idx_in_seg;
            if glyph_idx < self.glyph_id_array.len() {
                let raw = self.glyph_id_array[glyph_idx];
                if raw != 0 {
                    let glyph = (raw as u32).wrapping_add_signed(self.id_deltas[seg] as i32) as u16;
                    return Some(glyph);
                }
            }
            return None;
        }
        None
    }
}

/// Parse the cmap table. Returns a CmapTable with all supported subtables.
pub(crate) fn parse_cmap(data: &[u8]) -> Result<CmapTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("cmap table too short".into()));
    }
    let _version = u16::from_be_bytes([data[0], data[1]]);
    let num_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
    let header_size = 4usize;
    let record_size = 8usize;

    if data.len() < header_size + num_tables * record_size {
        return Err(FontError::InvalidFont(
            "cmap: encoding records overflow".into(),
        ));
    }

    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let off = header_size + i * record_size;
        let platform_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let encoding_id = u16::from_be_bytes([data[off + 2], data[off + 3]]);
        let subtable_offset =
            u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
        records.push(EncodingRecord {
            platform_id,
            encoding_id,
            subtable_offset,
        });
    }

    let mut fmt4 = Vec::new();
    let mut fmt12 = Vec::new();

    for rec in &records {
        let sub_off = rec.subtable_offset as usize;
        if sub_off + 2 > data.len() {
            continue;
        }
        let format = u16::from_be_bytes([data[sub_off], data[sub_off + 1]]);
        match format {
            4 => {
                if let Ok(sub) = parse_format4(data, sub_off) {
                    fmt4.push(Format4Subtable {
                        platform_id: rec.platform_id,
                        encoding_id: rec.encoding_id,
                        ..sub
                    });
                }
            }
            12 => {
                if let Ok(sub) = parse_format12(data, sub_off) {
                    fmt12.push(Format12Subtable {
                        platform_id: rec.platform_id,
                        encoding_id: rec.encoding_id,
                        ..sub
                    });
                }
            }
            other => {
                warn!("[cmap] unsupported format {}: skipping", other);
            }
        }
    }

    Ok(CmapTable {
        format4: fmt4,
        format12: fmt12,
    })
}

/// Parse a format 4 subtable. Returns the parsed fields.
fn parse_format4(data: &[u8], offset: usize) -> Result<Format4Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 4: too short".into()));
    }

    let length = u16::from_be_bytes([b[2], b[3]]) as usize;
    if offset + length > data.len() {
        return Err(FontError::InvalidFont(
            "cmap format 4: length exceeds data".into(),
        ));
    }
    let _language = u16::from_be_bytes([b[4], b[5]]);
    let seg_count_x2 = u16::from_be_bytes([b[6], b[7]]);
    let seg_count = (seg_count_x2 / 2) as usize;

    if seg_count == 0 {
        return Err(FontError::InvalidFont(
            "cmap format 4: zero segments".into(),
        ));
    }

    // Tables: endCode (seg_count), reservedPad (2), startCode (seg_count),
    //         idDelta (seg_count), idRangeOffset (seg_count), glyphIdArray (variable)
    let end_codes_off = 14usize;
    let start_codes_off = end_codes_off + seg_count * 2 + 2; // +2 for reservedPad
    let id_deltas_off = start_codes_off + seg_count * 2;
    let id_range_offsets_off = id_deltas_off + seg_count * 2;
    let glyph_array_off = id_range_offsets_off + seg_count * 2;

    let mut end_codes = Vec::with_capacity(seg_count);
    let mut start_codes = Vec::with_capacity(seg_count);
    let mut id_deltas = Vec::with_capacity(seg_count);
    let mut id_range_offsets = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let e = end_codes_off + i * 2;
        end_codes.push(u16::from_be_bytes([b[e], b[e + 1]]));
    }
    for i in 0..seg_count {
        let s = start_codes_off + i * 2;
        start_codes.push(u16::from_be_bytes([b[s], b[s + 1]]));
    }
    for i in 0..seg_count {
        let d = id_deltas_off + i * 2;
        id_deltas.push(i16::from_be_bytes([b[d], b[d + 1]]));
    }
    for i in 0..seg_count {
        let r = id_range_offsets_off + i * 2;
        id_range_offsets.push(u16::from_be_bytes([b[r], b[r + 1]]));
    }

    // glyphIdArray: variable length after id_range_offsets
    let mut glyph_id_array = Vec::new();
    let mut g_off = glyph_array_off;
    while g_off + 2 <= b.len() {
        glyph_id_array.push(u16::from_be_bytes([b[g_off], b[g_off + 1]]));
        g_off += 2;
    }

    Ok(Format4Subtable {
        platform_id: 0,
        encoding_id: 0,
        end_codes,
        start_codes,
        id_deltas,
        id_range_offsets,
        glyph_id_array,
    })
}

/// Parse a format 12 subtable.
fn parse_format12(data: &[u8], offset: usize) -> Result<Format12Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 12: too short".into()));
    }
    let _reserved = u16::from_be_bytes([b[2], b[3]]);
    let length = u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let _language = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
    let num_groups = u32::from_be_bytes([b[12], b[13], b[14], b[15]]) as usize;

    let group_start = 16usize;
    if group_start + num_groups * 12 > length || offset + length > data.len() {
        return Err(FontError::InvalidFont(
            "cmap format 12: groups overflow".into(),
        ));
    }

    let mut start_codes = Vec::with_capacity(num_groups);
    let mut end_codes = Vec::with_capacity(num_groups);
    let mut start_glyph_ids = Vec::with_capacity(num_groups);

    for i in 0..num_groups {
        let o = group_start + i * 12;
        let sc = u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let ec = u32::from_be_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]);
        let sg = u32::from_be_bytes([b[o + 8], b[o + 9], b[o + 10], b[o + 11]]);
        start_codes.push(sc);
        end_codes.push(ec);
        start_glyph_ids.push(sg);
    }

    Ok(Format12Subtable {
        platform_id: 0,
        encoding_id: 0,
        start_codes,
        end_codes,
        start_glyph_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_format4_segment(
        start: u16,
        end: u16,
        delta: i16,
        range_offset: u16,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let e = end.to_be_bytes().to_vec();
        let s = start.to_be_bytes().to_vec();
        let d = delta.to_be_bytes().to_vec();
        let r = range_offset.to_be_bytes().to_vec();
        (e, s, d, r, vec![])
    }

    fn build_format4_bytes(segments: &[(u16, u16, i16, u16, Vec<u16>)]) -> Vec<u8> {
        let seg_count = segments.len() as u16;
        let mut b = vec![0u8; 14]; // header
        b[0] = 0x00;
        b[1] = 0x04; // format 4
        let total_len = 14u16 + seg_count * 8 + 2; // header (14) + endCodes + reservedPad + startCodes + idDelta + idRangeOffset
        b[2] = (total_len >> 8) as u8;
        b[3] = total_len as u8;
        b[6] = ((seg_count * 2) >> 8) as u8;
        b[7] = (seg_count * 2) as u8;

        // Collect glyph arrays
        let mut end_codes = Vec::new();
        let mut start_codes = Vec::new();
        let mut id_deltas = Vec::new();
        let mut id_range_offsets = Vec::new();
        let mut glyph_array = Vec::new();
        for (s, e, d, r, ga) in segments {
            end_codes.extend_from_slice(&e.to_be_bytes());
            start_codes.extend_from_slice(&s.to_be_bytes());
            id_deltas.extend_from_slice(&d.to_be_bytes());
            id_range_offsets.extend_from_slice(&r.to_be_bytes());
            for g in ga {
                glyph_array.extend_from_slice(&g.to_be_bytes());
            }
        }
        b.extend(&end_codes);
        b.extend(&[0u8, 0u8]); // reservedPad
        b.extend(&start_codes);
        b.extend(&id_deltas);
        b.extend(&id_range_offsets);
        b.extend(&glyph_array);
        b
    }

    #[test]
    fn format4_segment_search_finds_code_in_first_segment() {
        let segments = vec![
            (32u16, 126u16, -32i16, 0u16, vec![]), // subtract 32 from char code
        ];
        let cmap_data_bytes = build_format4_bytes(&segments);

        // Wrap in cmap header: version=0, numTables=1, encoding record→offset 24
        let mut full_data = vec![0u8; 24]; // 4 (version+count) + 8 (record) + 12 (pad)
        full_data[2] = 0x00;
        full_data[3] = 0x01; // numTables = 1
        full_data[4..8].copy_from_slice(&[0x00, 0x03, 0x00, 0x01]); // platform 3, encoding 1
        let sub_off = 24u32;
        full_data[8..12].copy_from_slice(&sub_off.to_be_bytes());
        full_data.extend_from_slice(&cmap_data_bytes);

        let cmap = parse_cmap(&full_data).expect("should parse");
        assert_eq!(cmap.format4.len(), 1);
        // 'A' (65) should map to 65 - 32 = 33
        let glyph = cmap.map(65).expect("should map 'A'");
        assert_eq!(glyph, 33);
    }

    #[test]
    fn map_unmapped_codepoint_returns_none_not_error() {
        let segments = vec![(65u16, 90u16, 0i16, 0u16, vec![])]; // only A-Z
        let cmap_data_bytes = build_format4_bytes(&segments);
        let mut full_data = vec![0u8; 24];
        full_data[2] = 0x00;
        full_data[3] = 0x01;
        full_data[4..8].copy_from_slice(&[0x00, 0x03, 0x00, 0x01]);
        let sub_off = 24u32;
        full_data[8..12].copy_from_slice(&sub_off.to_be_bytes());
        full_data.extend_from_slice(&cmap_data_bytes);

        let cmap = parse_cmap(&full_data).expect("should parse");
        // '!' (33) is outside A-Z range → no mapping
        assert!(cmap.map(33).is_none());
    }
}
