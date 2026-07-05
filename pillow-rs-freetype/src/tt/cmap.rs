//! 'cmap' table — character to glyph index mapping.
//!
//! Faithful port of FreeType's `src/sfnt/ttcmap.c` format-4 and format-12
//! decoders (the Unicode subtables used by DejaVu/Liberation and the test fonts). The
//! `char_index` lookup reproduces `tt_cmap4_char_index` / `tt_cmap12_char_index`.

use crate::casts::{u16_from_i16, u16_from_u32};

use crate::error::FontError;
use log::warn;

/// A character-to-glyph mapping table holding all decoded Unicode subtables.
#[derive(Debug, Clone, Default)]
pub struct CmapTable {
    /// Parsed charmap records in font directory order.
    pub charmaps: Vec<CharmapRecord>,
    /// Format 4 subtables (BMP, U+0000–U+FFFF).
    pub format4: Vec<Format4Subtable>,
    /// Format 12 subtables (full Unicode, U+0000–U+10FFFF).
    pub format12: Vec<Format12Subtable>,
}

/// A selectable charmap entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharmapRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub format: u16,
    kind: CharmapKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharmapKind {
    Format4(usize),
    Format12(usize),
}

/// Format 4: Segment mapping for the Unicode BMP.
#[derive(Debug, Clone)]
pub struct Format4Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub end_codes: Vec<u16>,
    pub start_codes: Vec<u16>,
    pub id_deltas: Vec<i16>,
    pub id_range_offsets: Vec<u16>,
    /// Indexed via range offsets relative to the idRangeOffset slot itself.
    pub glyph_id_array: Vec<u16>,
}

/// Format 12: Segmented coverage for full Unicode.
#[derive(Debug, Clone)]
pub struct Format12Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub start_codes: Vec<u32>,
    pub end_codes: Vec<u32>,
    pub start_glyph_ids: Vec<u32>,
}

impl CmapTable {
    /// Map a Unicode codepoint to a glyph index, or `None` if unmapped.
    ///
    /// FreeType selects a single Unicode subtable at load time; we mirror
    /// the resolution priority (12 preferred over 4) and search all candidates.
    pub fn char_index(&self, codepoint: u32) -> Option<u16> {
        for sub in &self.format12 {
            if let Some(g) = sub.char_index(codepoint) {
                return Some(g);
            }
        }
        if codepoint <= 0xFFFF {
            for sub in &self.format4 {
                if let Some(g) = sub.char_index(u16_from_u32(codepoint)) {
                    return Some(g);
                }
            }
        }
        None
    }

    /// Map a codepoint with a specific selectable charmap.
    pub fn char_index_in_charmap(&self, charmap_index: usize, codepoint: u32) -> Option<u16> {
        match self.charmaps.get(charmap_index)?.kind {
            CharmapKind::Format4(index) => {
                if codepoint <= 0xFFFF {
                    self.format4[index].char_index(u16_from_u32(codepoint))
                } else {
                    None
                }
            }
            CharmapKind::Format12(index) => self.format12[index].char_index(codepoint),
        }
    }

    /// Return the first mapped codepoint and glyph index for a charmap.
    pub fn first_char(&self, charmap_index: usize) -> Option<(u32, u16)> {
        self.next_char(charmap_index, 0)
    }

    /// Return the next mapped codepoint strictly greater than `after`.
    pub fn next_char(&self, charmap_index: usize, after: u32) -> Option<(u32, u16)> {
        let record = self.charmaps.get(charmap_index)?;
        match record.kind {
            CharmapKind::Format4(index) => self.format4[index].next_char(after),
            CharmapKind::Format12(index) => self.format12[index].next_char(after),
        }
    }
}

impl Format12Subtable {
    fn char_index(&self, codepoint: u32) -> Option<u16> {
        for i in 0..self.start_codes.len() {
            if codepoint < self.start_codes[i] {
                break;
            }
            if codepoint <= self.end_codes[i] {
                return Some(u16_from_u32(
                    self.start_glyph_ids[i] + (codepoint - self.start_codes[i]),
                ));
            }
        }
        None
    }

    fn next_char(&self, after: u32) -> Option<(u32, u16)> {
        for i in 0..self.start_codes.len() {
            let candidate = self.start_codes[i].max(after.saturating_add(1));
            if candidate <= self.end_codes[i] {
                let glyph = self.start_glyph_ids[i] + (candidate - self.start_codes[i]);
                if glyph != 0 {
                    return Some((candidate, u16_from_u32(glyph)));
                }
            }
        }
        None
    }
}

impl Format4Subtable {
    /// Reproduce `tt_cmap4_char_index`: scan segments for the first whose
    /// `endCode >= charCode`, then resolve via delta or idRangeOffset.
    fn char_index(&self, char_code: u16) -> Option<u16> {
        for seg in 0..self.end_codes.len() {
            if char_code > self.end_codes[seg] {
                continue;
            }
            if char_code < self.start_codes[seg] {
                return None;
            }
            if self.id_range_offsets[seg] == 0 {
                // glyph = (charCode + delta) mod 65536
                return Some(char_code.wrapping_add(u16_from_i16(self.id_deltas[seg])));
            }
            // idRangeOffset semantics: the offset is *relative to the address of
            // this very idRangeOffset entry*. C does `glyph_id_array[
            //   idRangeOffset[i]/2 + (c - startCode[i]) - (numSeg - i) ]`.
            // We store idRangeOffset relative to the start of the idRangeOffset
            // array (see parse_format4), so index into glyph_id_array is:
            let ro = self.id_range_offsets[seg] as usize / 2;
            let idx = ro
                .wrapping_sub(self.id_range_offsets.len() - seg)
                .wrapping_add((char_code - self.start_codes[seg]) as usize);
            if let Some(&raw) = self.glyph_id_array.get(idx) {
                if raw == 0 {
                    return None;
                }
                return Some(raw.wrapping_add(u16_from_i16(self.id_deltas[seg])));
            }
            return None;
        }
        None
    }

    fn next_char(&self, after: u32) -> Option<(u32, u16)> {
        let start = after.saturating_add(1);
        if start > u16::MAX as u32 {
            return None;
        }
        let mut cp = u16_from_u32(start);
        loop {
            if cp == 0xFFFF {
                return None;
            }
            if let Some(glyph) = self.char_index(cp) {
                if glyph != 0 {
                    return Some((cp as u32, glyph));
                }
            }
            cp = cp.wrapping_add(1);
        }
    }
}

/// Parse the 'cmap' table.
pub fn parse_cmap(data: &[u8]) -> Result<CmapTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("cmap table too short".into()));
    }
    let num_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
    if data.len() < 4 + num_tables * 8 {
        return Err(FontError::InvalidFont(
            "cmap: encoding records overflow".into(),
        ));
    }

    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let off = 4 + i * 8;
        records.push((
            u16::from_be_bytes([data[off], data[off + 1]]),
            u16::from_be_bytes([data[off + 2], data[off + 3]]),
            u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]),
        ));
    }

    let mut table = CmapTable::default();
    for (platform_id, encoding_id, sub_off) in &records {
        let sub_off = *sub_off as usize;
        if sub_off + 2 > data.len() {
            continue;
        }
        let format = u16::from_be_bytes([data[sub_off], data[sub_off + 1]]);
        match format {
            4 => match parse_format4(data, sub_off, *platform_id, *encoding_id) {
                Ok(sub) => {
                    let index = table.format4.len();
                    table.format4.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        kind: CharmapKind::Format4(index),
                    });
                }
                Err(e) => warn!("[cmap] format 4 parse failed: {e}"),
            },
            12 => match parse_format12(data, sub_off, *platform_id, *encoding_id) {
                Ok(sub) => {
                    let index = table.format12.len();
                    table.format12.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        kind: CharmapKind::Format12(index),
                    });
                }
                Err(e) => warn!("[cmap] format 12 parse failed: {e}"),
            },
            other => warn!("[cmap] unsupported format {other}: skipping"),
        }
    }

    Ok(table)
}

fn parse_format4(
    data: &[u8],
    offset: usize,
    platform_id: u16,
    encoding_id: u16,
) -> Result<Format4Subtable, FontError> {
    let b = data.get(offset..).ok_or_else(|| {
        FontError::InvalidFont("cmap format 4: subtable offset out of range".into())
    })?;
    if b.len() < 14 {
        return Err(FontError::InvalidFont("cmap format 4: too short".into()));
    }
    let length = u16::from_be_bytes([b[2], b[3]]) as usize;
    let body = data
        .get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidFont("cmap format 4: length exceeds data".into()))?;
    let seg_count = (u16::from_be_bytes([body[6], body[7]]) / 2) as usize;
    if seg_count == 0 {
        return Err(FontError::InvalidFont(
            "cmap format 4: zero segments".into(),
        ));
    }

    let end_off = 14usize;
    let start_off = end_off + seg_count * 2 + 2; // +2 reservedPad
    let delta_off = start_off + seg_count * 2;
    let range_off = delta_off + seg_count * 2;
    let glyph_off = range_off + seg_count * 2;

    let mut end_codes = Vec::with_capacity(seg_count);
    let mut start_codes = Vec::with_capacity(seg_count);
    let mut id_deltas = Vec::with_capacity(seg_count);
    let mut id_range_offsets = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        end_codes.push(u16::from_be_bytes([
            body[end_off + i * 2],
            body[end_off + i * 2 + 1],
        ]));
        start_codes.push(u16::from_be_bytes([
            body[start_off + i * 2],
            body[start_off + i * 2 + 1],
        ]));
        id_deltas.push(i16::from_be_bytes([
            body[delta_off + i * 2],
            body[delta_off + i * 2 + 1],
        ]));
        id_range_offsets.push(u16::from_be_bytes([
            body[range_off + i * 2],
            body[range_off + i * 2 + 1],
        ]));
    }

    let mut glyph_id_array = Vec::new();
    let mut g = glyph_off;
    while g + 2 <= body.len() {
        glyph_id_array.push(u16::from_be_bytes([body[g], body[g + 1]]));
        g += 2;
    }

    Ok(Format4Subtable {
        platform_id,
        encoding_id,
        end_codes,
        start_codes,
        id_deltas,
        id_range_offsets,
        glyph_id_array,
    })
}

fn parse_format12(
    data: &[u8],
    offset: usize,
    platform_id: u16,
    encoding_id: u16,
) -> Result<Format12Subtable, FontError> {
    let body = data
        .get(offset..)
        .ok_or_else(|| FontError::InvalidFont("cmap format 12: offset out of range".into()))?;
    if body.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 12: too short".into()));
    }
    let num_groups = u32::from_be_bytes([body[12], body[13], body[14], body[15]]) as usize;
    if 16 + num_groups * 12 > body.len() {
        return Err(FontError::InvalidFont(
            "cmap format 12: groups overflow".into(),
        ));
    }

    let mut start_codes = Vec::with_capacity(num_groups);
    let mut end_codes = Vec::with_capacity(num_groups);
    let mut start_glyph_ids = Vec::with_capacity(num_groups);
    for i in 0..num_groups {
        let o = 16 + i * 12;
        start_codes.push(u32::from_be_bytes([
            body[o],
            body[o + 1],
            body[o + 2],
            body[o + 3],
        ]));
        end_codes.push(u32::from_be_bytes([
            body[o + 4],
            body[o + 5],
            body[o + 6],
            body[o + 7],
        ]));
        start_glyph_ids.push(u32::from_be_bytes([
            body[o + 8],
            body[o + 9],
            body[o + 10],
            body[o + 11],
        ]));
    }

    Ok(Format12Subtable {
        platform_id,
        encoding_id,
        start_codes,
        end_codes,
        start_glyph_ids,
    })
}
