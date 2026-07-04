//! 'name' table — Naming Table. Mirrors `tt_face_load_name`.
//!
//! Reference: `src/sfnt/ttload.c`. Prefers platform 3 (Windows) / encoding 1.

use crate::error::FontError;

/// Font identification strings.
#[derive(Debug, Clone)]
pub struct NameTable {
    /// Font family name (nameID 1).
    pub family: String,
    /// Font subfamily / style name (nameID 2).
    pub subfamily: String,
}

/// nameID constants we read.
const NAME_ID_FAMILY: u16 = 1;
const NAME_ID_SUBFAMILY: u16 = 2;
const NAME_ID_TYPO_FAMILY: u16 = 16;
const NAME_ID_TYPO_SUBFAMILY: u16 = 17;

#[derive(Debug)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    name_id: u16,
    offset: u16,
    length: u16,
}

/// Parse the 'name' table.
pub fn parse_name(data: &[u8]) -> Result<NameTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "name table too short (need 6 bytes)".into(),
        ));
    }
    let count = u16::from_be_bytes([data[2], data[3]]) as usize;
    let string_offset = u16::from_be_bytes([data[4], data[5]]) as usize;

    if data.len() < 6 + count * 12 {
        return Err(FontError::InvalidFont(
            "name table: records overflow data".into(),
        ));
    }

    let mut records = Vec::with_capacity(count);
    for i in 0..count {
        let off = 6 + i * 12;
        records.push(NameRecord {
            platform_id: u16::from_be_bytes([data[off], data[off + 1]]),
            encoding_id: u16::from_be_bytes([data[off + 2], data[off + 3]]),
            name_id: u16::from_be_bytes([data[off + 6], data[off + 7]]),
            length: u16::from_be_bytes([data[off + 8], data[off + 9]]),
            offset: u16::from_be_bytes([data[off + 10], data[off + 11]]),
        });
    }

    // Prefer typographic family/subfamily (nameID 16/17) over legacy (1/2).
    // FreeType 2.14.3 uses typographic names when available via face->family_name
    // which checks nameID 16 first, falling back to nameID 1.
    let family = find_name_string(data, string_offset, &records, NAME_ID_TYPO_FAMILY)
        .or_else(|| find_name_string(data, string_offset, &records, NAME_ID_FAMILY))
        .unwrap_or_else(|| "Unknown".into());
    let subfamily = find_name_string(data, string_offset, &records, NAME_ID_TYPO_SUBFAMILY)
        .or_else(|| find_name_string(data, string_offset, &records, NAME_ID_SUBFAMILY))
        .unwrap_or_else(|| "Regular".into());

    Ok(NameTable { family, subfamily })
}

/// Search for a name string by name_id, preferring platform 3/encoding 1.
fn find_name_string(
    data: &[u8],
    string_base: usize,
    records: &[NameRecord],
    name_id: u16,
) -> Option<String> {
    // Priority 1: platform 3 (Windows), encoding 1 (Unicode BMP, UTF-16BE).
    for r in records {
        if r.name_id == name_id && r.platform_id == 3 && r.encoding_id == 1 {
            if let Ok(s) = decode_utf16be(data, string_base, r) {
                return Some(s);
            }
        }
    }
    // Priority 2: platform 1 (Mac), encoding 0 (Roman) — ASCII subset.
    for r in records {
        if r.name_id == name_id && r.platform_id == 1 && r.encoding_id == 0 {
            if let Ok(s) = decode_mac_roman(data, string_base, r) {
                return Some(s);
            }
        }
    }
    // Fallback: any platform-3 record.
    for r in records {
        if r.name_id == name_id && r.platform_id == 3 {
            if let Ok(s) = decode_utf16be(data, string_base, r) {
                return Some(s);
            }
        }
    }
    None
}

fn decode_utf16be(data: &[u8], base: usize, r: &NameRecord) -> Result<String, FontError> {
    let start = base + r.offset as usize;
    let end = start + r.length as usize;
    let bytes = data
        .get(start..end)
        .ok_or_else(|| FontError::InvalidFont("name: string offset out of range".into()))?;
    if !r.length.is_multiple_of(2) {
        return Err(FontError::InvalidFont(
            "name: UTF-16BE string has odd length".into(),
        ));
    }
    let chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&chars)
        .map_err(|e| FontError::InvalidFont(format!("name: invalid UTF-16: {e}")))
}

fn decode_mac_roman(data: &[u8], base: usize, r: &NameRecord) -> Result<String, FontError> {
    let start = base + r.offset as usize;
    let end = start + r.length as usize;
    let bytes = data.get(start..end).ok_or_else(|| {
        FontError::InvalidFont("name: Mac Roman string offset out of range".into())
    })?;
    // Mac Roman 0x00–0x7F is ASCII; higher bytes would need a table (rare in test fonts).
    Ok(bytes.iter().map(|&b| b as char).collect())
}
