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
    /// Format 6 subtables (trimmed 16-bit code range, often Macintosh encodings).
    pub format6: Vec<Format6Subtable>,
    /// Format 12 subtables (full Unicode, U+0000–U+10FFFF).
    pub format12: Vec<Format12Subtable>,
    /// Format 14 Unicode variation selector subtables.
    pub format14: Vec<Format14Subtable>,
}

/// A selectable charmap entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharmapRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub format: u16,
    pub language_id: u32,
    kind: CharmapKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharmapKind {
    Format4(usize),
    Format6(usize),
    Format12(usize),
    Format14,
}

/// Format 4: Segment mapping for the Unicode BMP.
#[derive(Debug, Clone)]
pub struct Format4Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub end_codes: Vec<u16>,
    pub start_codes: Vec<u16>,
    pub id_deltas: Vec<i16>,
    pub id_range_offsets: Vec<u16>,
    /// Indexed via range offsets relative to the idRangeOffset slot itself.
    pub glyph_id_array: Vec<u16>,
}

/// Format 6: Trimmed table mapping a contiguous 16-bit code range to glyph IDs.
#[derive(Debug, Clone)]
pub struct Format6Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub first_code: u16,
    pub glyph_id_array: Vec<u16>,
}

/// Format 12: Segmented coverage for full Unicode.
#[derive(Debug, Clone)]
pub struct Format12Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u32,
    pub start_codes: Vec<u32>,
    pub end_codes: Vec<u32>,
    pub start_glyph_ids: Vec<u32>,
}

/// Format 14: Unicode variation selector metadata.
#[derive(Debug, Clone)]
pub struct Format14Subtable {
    pub platform_id: u16,
    pub encoding_id: u16,
    records: Vec<VariationSelectorRecord>,
}

#[derive(Debug, Clone)]
struct VariationSelectorRecord {
    selector: u32,
    default_ranges: Vec<VariationDefaultRange>,
    non_default_mappings: Vec<VariationMapping>,
}

#[derive(Debug, Clone)]
struct VariationDefaultRange {
    start: u32,
    end: u32,
}

#[derive(Debug, Clone)]
struct VariationMapping {
    codepoint: u32,
    glyph_id: u16,
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
            CharmapKind::Format6(index) => {
                if codepoint <= 0xFFFF {
                    self.format6[index].char_index(u16_from_u32(codepoint))
                } else {
                    None
                }
            }
            CharmapKind::Format12(index) => self.format12[index].char_index(codepoint),
            CharmapKind::Format14 => None,
        }
    }

    /// Equivalent to `FT_Face_GetCharVariantIndex` for the active Unicode charmap.
    pub fn char_variant_index(
        &self,
        charmap_index: usize,
        codepoint: u32,
        variant_selector: u32,
    ) -> u16 {
        let Some(record) = self.charmaps.get(charmap_index) else {
            return 0;
        };
        if !record.is_unicode() {
            return 0;
        }
        for subtable in &self.format14 {
            if let Some(glyph) = subtable.char_variant_index(codepoint, variant_selector, |cp| {
                self.char_index_in_charmap(charmap_index, cp).unwrap_or(0)
            }) {
                return glyph;
            }
        }
        0
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
            CharmapKind::Format6(index) => self.format6[index].next_char(after),
            CharmapKind::Format12(index) => self.format12[index].next_char(after),
            CharmapKind::Format14 => None,
        }
    }
}

impl CharmapRecord {
    fn is_unicode(&self) -> bool {
        self.platform_id == 0 || (self.platform_id == 3 && matches!(self.encoding_id, 1 | 10))
    }
}

impl Format6Subtable {
    fn char_index(&self, char_code: u16) -> Option<u16> {
        let offset = char_code.checked_sub(self.first_code)? as usize;
        self.glyph_id_array
            .get(offset)
            .copied()
            .filter(|glyph| *glyph != 0)
    }

    fn next_char(&self, after: u32) -> Option<(u32, u16)> {
        // Unlike formats 4 and 12, `tt_cmap6_char_next` advances its
        // FT_UInt32 before checking the terminal value, so it wraps.
        let start = after.wrapping_add(1).max(u32::from(self.first_code));
        if start > u16::MAX as u32 {
            return None;
        }
        let mut char_code = u16_from_u32(start);
        loop {
            let offset = char_code - self.first_code;
            let &glyph = self.glyph_id_array.get(offset as usize)?;
            if glyph != 0 {
                return Some((u32::from(char_code), glyph));
            }
            if char_code == u16::MAX {
                return None;
            }
            char_code = char_code.wrapping_add(1);
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
            let candidate = self.start_codes[i].max(after.checked_add(1)?);
            if candidate <= self.end_codes[i] {
                let glyph = self.start_glyph_ids[i] + (candidate - self.start_codes[i]);
                if glyph != 0 {
                    return Some((candidate, u16_from_u32(glyph)));
                }
                // Format 12 groups increment glyph IDs with codepoints.  A
                // zero group start is skipped, not the remainder of the group.
                if candidate < self.end_codes[i] {
                    return Some((candidate + 1, 1));
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
        let last = self.end_codes.len() - 1;
        for seg in 0..last {
            if char_code > self.end_codes[seg] {
                continue;
            }
            return self.char_index_in_segment(seg, char_code);
        }
        self.char_index_in_segment(last, char_code)
    }

    fn char_index_in_segment(&self, seg: usize, char_code: u16) -> Option<u16> {
        if char_code < self.start_codes[seg] {
            return None;
        }
        if self.id_range_offsets[seg] == 0 {
            return Some(char_code.wrapping_add(u16_from_i16(self.id_deltas[seg])));
        }
        // `idRangeOffset` is relative to its own array entry.
        let ro = self.id_range_offsets[seg] as usize / 2;
        let idx =
            ro - (self.id_range_offsets.len() - seg) + (char_code - self.start_codes[seg]) as usize;
        let raw = self.glyph_id_array[idx];
        (raw != 0).then(|| raw.wrapping_add(u16_from_i16(self.id_deltas[seg])))
    }

    fn next_char(&self, after: u32) -> Option<(u32, u16)> {
        if after >= u32::from(u16::MAX) {
            return None;
        }
        let start = after + 1;
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

impl Format14Subtable {
    fn char_variant_index(
        &self,
        codepoint: u32,
        variant_selector: u32,
        mut unicode_index: impl FnMut(u32) -> u16,
    ) -> Option<u16> {
        let record = self
            .records
            .iter()
            .find(|record| record.selector == variant_selector)?;
        if record
            .default_ranges
            .iter()
            .any(|range| codepoint >= range.start && codepoint <= range.end)
        {
            return Some(unicode_index(codepoint));
        }
        record
            .non_default_mappings
            .iter()
            .find(|mapping| mapping.codepoint == codepoint)
            .map(|mapping| mapping.glyph_id)
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
                    let language_id = sub.language_id;
                    table.format4.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        language_id: u32::from(language_id),
                        kind: CharmapKind::Format4(index),
                    });
                }
                Err(e) => warn!("[cmap] format 4 parse failed: {e}"),
            },
            6 => match parse_format6(data, sub_off, *platform_id, *encoding_id) {
                Ok(sub) => {
                    let index = table.format6.len();
                    let language_id = sub.language_id;
                    table.format6.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        language_id: u32::from(language_id),
                        kind: CharmapKind::Format6(index),
                    });
                }
                Err(e) => warn!("[cmap] format 6 parse failed: {e}"),
            },
            12 => match parse_format12(data, sub_off, *platform_id, *encoding_id) {
                Ok(sub) => {
                    let index = table.format12.len();
                    let language_id = sub.language_id;
                    table.format12.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        language_id,
                        kind: CharmapKind::Format12(index),
                    });
                }
                Err(e) => warn!("[cmap] format 12 parse failed: {e}"),
            },
            14 => match parse_format14(data, sub_off, *platform_id, *encoding_id) {
                Ok(sub) => {
                    table.format14.push(sub);
                    table.charmaps.push(CharmapRecord {
                        platform_id: *platform_id,
                        encoding_id: *encoding_id,
                        format,
                        // FreeType `tt_cmap14_get_info` reports no language
                        // field with this sentinel (`src/sfnt/ttcmap.c`).
                        language_id: 0xFFFF_FFFF,
                        kind: CharmapKind::Format14,
                    });
                }
                Err(e) => warn!("[cmap] format 14 parse failed: {e}"),
            },
            other => {
                // FreeType only exposes charmaps whose format class loads.
                warn!("[cmap] unsupported format {other}: ignoring charmap record");
            }
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
    let b = &data[offset..];
    if b.len() < 14 {
        return Err(FontError::InvalidFont("cmap format 4: too short".into()));
    }
    let length = u16::from_be_bytes([b[2], b[3]]) as usize;
    let body = data
        .get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidFont("cmap format 4: length exceeds data".into()))?;
    let language_id = u16::from_be_bytes([body[4], body[5]]);
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
    if body.len() < glyph_off {
        return Err(FontError::InvalidFont(
            "cmap format 4: segment arrays exceed length".into(),
        ));
    }

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

    if end_codes.last() != Some(&u16::MAX) {
        return Err(FontError::InvalidFont(
            "cmap format 4: missing terminal segment".into(),
        ));
    }

    for seg in 0..seg_count {
        let range_offset = id_range_offsets[seg];
        if range_offset == 0 {
            continue;
        }
        let Some(span) = end_codes[seg].checked_sub(start_codes[seg]) else {
            return Err(FontError::InvalidFont(
                "cmap format 4: segment end precedes start".into(),
            ));
        };
        let Some(first) = (range_offset as usize / 2).checked_sub(seg_count - seg) else {
            return Err(FontError::InvalidFont(
                "cmap format 4: range offset precedes glyph array".into(),
            ));
        };
        if first + span as usize >= glyph_id_array.len() {
            return Err(FontError::InvalidFont(
                "cmap format 4: range offset exceeds glyph array".into(),
            ));
        }
    }

    Ok(Format4Subtable {
        platform_id,
        encoding_id,
        language_id,
        end_codes,
        start_codes,
        id_deltas,
        id_range_offsets,
        glyph_id_array,
    })
}

fn parse_format6(
    data: &[u8],
    offset: usize,
    platform_id: u16,
    encoding_id: u16,
) -> Result<Format6Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 10 {
        return Err(FontError::InvalidFont("cmap format 6: too short".into()));
    }
    let length = u16::from_be_bytes([b[2], b[3]]) as usize;
    let body = data
        .get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidFont("cmap format 6: length exceeds data".into()))?;
    if body.len() < 10 {
        return Err(FontError::InvalidFont(
            "cmap format 6: length too short".into(),
        ));
    }
    let language_id = u16::from_be_bytes([body[4], body[5]]);
    let first_code = u16::from_be_bytes([body[6], body[7]]);
    let entry_count = u16::from_be_bytes([body[8], body[9]]) as usize;
    if 10 + entry_count * 2 > body.len() {
        return Err(FontError::InvalidFont(
            "cmap format 6: glyph array exceeds length".into(),
        ));
    }
    let mut glyph_id_array = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let off = 10 + i * 2;
        glyph_id_array.push(u16::from_be_bytes([body[off], body[off + 1]]));
    }

    Ok(Format6Subtable {
        platform_id,
        encoding_id,
        language_id,
        first_code,
        glyph_id_array,
    })
}

fn parse_format12(
    data: &[u8],
    offset: usize,
    platform_id: u16,
    encoding_id: u16,
) -> Result<Format12Subtable, FontError> {
    let body = &data[offset..];
    if body.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 12: too short".into()));
    }
    let language_id = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
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
        language_id,
        start_codes,
        end_codes,
        start_glyph_ids,
    })
}

fn parse_format14(
    data: &[u8],
    offset: usize,
    platform_id: u16,
    encoding_id: u16,
) -> Result<Format14Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 10 {
        return Err(FontError::InvalidFont("cmap format 14: too short".into()));
    }
    let length = u32::from_be_bytes([b[2], b[3], b[4], b[5]]) as usize;
    if length < 10 {
        return Err(FontError::InvalidFont(
            "cmap format 14: length too short".into(),
        ));
    }
    let body = data
        .get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidFont("cmap format 14: length exceeds data".into()))?;
    let num_records = u32::from_be_bytes([body[6], body[7], body[8], body[9]]) as usize;
    let selector_records_len = num_records
        .checked_mul(11)
        .and_then(|len| 10usize.checked_add(len))
        .ok_or_else(|| {
            FontError::InvalidFont(
                "cmap format 14: variation selector records overflow length".into(),
            )
        })?;
    if selector_records_len > body.len() {
        return Err(FontError::InvalidFont(
            "cmap format 14: variation selector records exceed length".into(),
        ));
    }
    let mut records = Vec::with_capacity(num_records);
    let mut last_selector = 1u32;
    for i in 0..num_records {
        let record_offset = 10 + i * 11;
        let selector = read_u24(&body[record_offset..record_offset + 3]);
        let default_offset = u32::from_be_bytes([
            body[record_offset + 3],
            body[record_offset + 4],
            body[record_offset + 5],
            body[record_offset + 6],
        ]) as usize;
        let non_default_offset = u32::from_be_bytes([
            body[record_offset + 7],
            body[record_offset + 8],
            body[record_offset + 9],
            body[record_offset + 10],
        ]) as usize;
        if default_offset >= length || non_default_offset >= length {
            return Err(FontError::InvalidFont(
                "cmap format 14: UVS table offset out of range".into(),
            ));
        }
        if selector < last_selector {
            return Err(FontError::InvalidFont(
                "cmap format 14: variation selectors out of order".into(),
            ));
        }
        last_selector = selector.saturating_add(1);
        let default_ranges = if default_offset == 0 {
            Vec::new()
        } else {
            parse_format14_default_ranges(body, default_offset)?
        };
        let non_default_mappings = if non_default_offset == 0 {
            Vec::new()
        } else {
            parse_format14_non_default_mappings(body, non_default_offset)?
        };
        records.push(VariationSelectorRecord {
            selector,
            default_ranges,
            non_default_mappings,
        });
    }
    Ok(Format14Subtable {
        platform_id,
        encoding_id,
        records,
    })
}

fn parse_format14_default_ranges(
    body: &[u8],
    offset: usize,
) -> Result<Vec<VariationDefaultRange>, FontError> {
    let count_bytes = body.get(offset..offset + 4).ok_or_else(|| {
        FontError::InvalidFont("cmap format 14: default UVS count missing".into())
    })?;
    let count = u32::from_be_bytes([
        count_bytes[0],
        count_bytes[1],
        count_bytes[2],
        count_bytes[3],
    ]) as usize;
    let records_offset = offset + 4;
    let records_len = count
        .checked_mul(4)
        .and_then(|len| records_offset.checked_add(len))
        .ok_or_else(|| {
            FontError::InvalidFont("cmap format 14: default UVS records overflow length".into())
        })?;
    if records_len > body.len() {
        return Err(FontError::InvalidFont(
            "cmap format 14: default UVS records exceed length".into(),
        ));
    }
    let mut ranges = Vec::with_capacity(count);
    let mut last_base = 0;
    for i in 0..count {
        let record_offset = records_offset + i * 4;
        let start = read_u24(&body[record_offset..record_offset + 3]);
        let additional = u32::from(body[record_offset + 3]);
        let end = start + additional;
        if end >= 0x11_0000 {
            return Err(FontError::InvalidFont(
                "cmap format 14: default UVS range exceeds Unicode".into(),
            ));
        }
        if start < last_base {
            return Err(FontError::InvalidFont(
                "cmap format 14: default UVS ranges out of order".into(),
            ));
        }
        last_base = end + 1;
        ranges.push(VariationDefaultRange { start, end });
    }
    Ok(ranges)
}

fn parse_format14_non_default_mappings(
    body: &[u8],
    offset: usize,
) -> Result<Vec<VariationMapping>, FontError> {
    let count_bytes = body.get(offset..offset + 4).ok_or_else(|| {
        FontError::InvalidFont("cmap format 14: non-default UVS count missing".into())
    })?;
    let count = u32::from_be_bytes([
        count_bytes[0],
        count_bytes[1],
        count_bytes[2],
        count_bytes[3],
    ]) as usize;
    let records_offset = offset + 4;
    let records_len = count
        .checked_mul(5)
        .and_then(|len| records_offset.checked_add(len))
        .ok_or_else(|| {
            FontError::InvalidFont("cmap format 14: non-default UVS records overflow length".into())
        })?;
    if records_len > body.len() {
        return Err(FontError::InvalidFont(
            "cmap format 14: non-default UVS records exceed length".into(),
        ));
    }
    let mut mappings = Vec::with_capacity(count);
    let mut last_codepoint = 0;
    for i in 0..count {
        let record_offset = records_offset + i * 5;
        let codepoint = read_u24(&body[record_offset..record_offset + 3]);
        let glyph_id = u16::from_be_bytes([body[record_offset + 3], body[record_offset + 4]]);
        if codepoint >= 0x11_0000 {
            return Err(FontError::InvalidFont(
                "cmap format 14: non-default UVS codepoint exceeds Unicode".into(),
            ));
        }
        if codepoint < last_codepoint {
            return Err(FontError::InvalidFont(
                "cmap format 14: non-default UVS mappings out of order".into(),
            ));
        }
        last_codepoint = codepoint + 1;
        mappings.push(VariationMapping {
            codepoint,
            glyph_id,
        });
    }
    Ok(mappings)
}

fn read_u24(bytes: &[u8]) -> u32 {
    (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2])
}
