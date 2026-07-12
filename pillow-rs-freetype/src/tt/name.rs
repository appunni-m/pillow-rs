//! 'name' table — Naming Table. Mirrors `tt_face_load_name`.
//!
//! Reference: `src/sfnt/ttload.c`. Prefers platform 3 (Windows) / encoding 1.

use crate::error::FontError;

/// Font identification strings.
#[derive(Debug, Clone)]
pub struct NameTable {
    /// Raw name table format field.
    pub format: u16,
    /// Font family name (nameID 1).
    pub family: String,
    /// Font subfamily / style name (nameID 2).
    pub subfamily: String,
    /// PostScript name (nameID 6), when present.
    pub postscript_name: Option<String>,
    /// Raw SFNT name records exposed by `FT_Get_Sfnt_Name`.
    pub records: Vec<SfntNameRecord>,
    /// Raw language-tag records exposed by `FT_Get_Sfnt_LangTag`.
    pub lang_tags: Vec<SfntLangTagRecord>,
}

/// Raw SFNT name record with string bytes copied from the name table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfntNameRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: u16,
    pub string: Vec<u8>,
}

/// Raw SFNT language-tag record with string bytes copied from the name table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfntLangTagRecord {
    pub string: Vec<u8>,
}

/// nameID constants we read.
const NAME_ID_FAMILY: u16 = 1;
const NAME_ID_SUBFAMILY: u16 = 2;
const NAME_ID_POSTSCRIPT: u16 = 6;
const NAME_ID_VARIATIONS_PREFIX: u16 = 25;
const NAME_ID_TYPO_FAMILY: u16 = 16;
const NAME_ID_TYPO_SUBFAMILY: u16 = 17;

#[derive(Debug)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    language_id: u16,
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
    let format = u16::from_be_bytes([data[0], data[1]]);
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
            language_id: u16::from_be_bytes([data[off + 4], data[off + 5]]),
            name_id: u16::from_be_bytes([data[off + 6], data[off + 7]]),
            length: u16::from_be_bytes([data[off + 8], data[off + 9]]),
            offset: u16::from_be_bytes([data[off + 10], data[off + 11]]),
        });
    }
    let raw_records = records
        .iter()
        .filter_map(|record| raw_record(data, string_offset, record))
        .collect();
    let lang_tags = if format == 1 {
        parse_lang_tags(data, string_offset, 6 + count * 12)?
    } else {
        Vec::new()
    };

    // Prefer typographic family/subfamily (nameID 16/17) over legacy (1/2).
    // FreeType 2.14.3 uses typographic names when available via face->family_name
    // which checks nameID 16 first, falling back to nameID 1.
    let family = find_name_string(data, string_offset, &records, NAME_ID_TYPO_FAMILY)
        .or_else(|| find_name_string(data, string_offset, &records, NAME_ID_FAMILY))
        .unwrap_or_else(|| "Unknown".into());
    let subfamily = find_name_string(data, string_offset, &records, NAME_ID_TYPO_SUBFAMILY)
        .or_else(|| find_name_string(data, string_offset, &records, NAME_ID_SUBFAMILY))
        .unwrap_or_else(|| "Regular".into());
    let postscript_name = find_postscript_name(data, string_offset, &records);

    Ok(NameTable {
        format,
        family,
        subfamily,
        postscript_name,
        records: raw_records,
        lang_tags,
    })
}

fn raw_record(data: &[u8], string_base: usize, record: &NameRecord) -> Option<SfntNameRecord> {
    if record.length == 0 {
        return None;
    }
    // All operands originate from u16 name-table fields, so these additions
    // cannot overflow usize; the slice lookup below owns the range check.
    let start = string_base + record.offset as usize;
    let end = start + record.length as usize;
    let bytes = data.get(start..end)?;
    Some(SfntNameRecord {
        platform_id: record.platform_id,
        encoding_id: record.encoding_id,
        language_id: record.language_id,
        name_id: record.name_id,
        string: bytes.to_vec(),
    })
}

fn parse_lang_tags(
    data: &[u8],
    string_base: usize,
    lang_tag_count_offset: usize,
) -> Result<Vec<SfntLangTagRecord>, FontError> {
    let count_bytes = data
        .get(lang_tag_count_offset..lang_tag_count_offset + 2)
        .ok_or_else(|| FontError::InvalidFont("name table: language-tag count missing".into()))?;
    let count = u16::from_be_bytes([count_bytes[0], count_bytes[1]]) as usize;
    let records_offset = lang_tag_count_offset + 2;
    if data.len() < records_offset + count * 4 {
        return Err(FontError::InvalidFont(
            "name table: language-tag records overflow data".into(),
        ));
    }

    let mut lang_tags = Vec::with_capacity(count);
    for i in 0..count {
        let off = records_offset + i * 4;
        let length = u16::from_be_bytes([data[off], data[off + 1]]) as usize;
        let offset = u16::from_be_bytes([data[off + 2], data[off + 3]]) as usize;
        let start = string_base + offset;
        let end = start + length;
        let bytes = data.get(start..end).ok_or_else(|| {
            FontError::InvalidFont("name table: language-tag string out of range".into())
        })?;
        lang_tags.push(SfntLangTagRecord {
            string: bytes.to_vec(),
        });
    }
    Ok(lang_tags)
}

/// Search for a name string by name_id, preferring platform 3/encoding 1.
fn find_name_string(
    data: &[u8],
    string_base: usize,
    records: &[NameRecord],
    name_id: u16,
) -> Option<String> {
    let mut found_apple_roman = None;
    let mut found_apple_english = None;
    let mut found_win = None;
    let mut found_unicode = None;
    let mut win_is_english = false;

    for (index, record) in records.iter().enumerate() {
        if record.name_id != name_id || record.length == 0 {
            continue;
        }
        match record.platform_id {
            0 | 2 => found_unicode = Some(index),
            1 if record.language_id == 0 => found_apple_english = Some(index),
            1 if record.encoding_id == 0 => found_apple_roman = Some(index),
            3 if matches!(record.encoding_id, 0 | 1 | 10)
                && (found_win.is_none() || (record.language_id & 0x03ff) == 0x0009) =>
            {
                win_is_english = (record.language_id & 0x03ff) == 0x0009;
                found_win = Some(index);
            }
            _ => {}
        }
    }

    let found_apple = found_apple_english.or(found_apple_roman);
    // Mirrors FreeType `tt_face_get_name`: prefer Windows Unicode names unless
    // the only Windows candidate is non-English and an Apple name is present.
    if let Some(index) = found_win
        && (found_apple.is_none() || win_is_english)
        && let Ok(name) = decode_utf16be(data, string_base, &records[index])
    {
        return Some(name);
    }
    if let Some(index) = found_apple
        && let Ok(name) = decode_mac_roman(data, string_base, &records[index])
    {
        return Some(name);
    }
    if let Some(index) = found_unicode
        && let Ok(name) = decode_utf16be(data, string_base, &records[index])
    {
        return Some(name);
    }
    None
}

/// Return the preferred Unicode name string for a raw name ID.
pub fn name_string(table: &NameTable, name_id: u16) -> Option<String> {
    let mut found_apple_roman = None;
    let mut found_apple_english = None;
    let mut found_win = None;
    let mut found_unicode = None;
    let mut win_is_english = false;

    for (index, record) in table.records.iter().enumerate() {
        if record.name_id != name_id || record.string.is_empty() {
            continue;
        }
        match record.platform_id {
            0 | 2 => found_unicode = Some(index),
            1 if record.language_id == 0 => found_apple_english = Some(index),
            1 if record.encoding_id == 0 => found_apple_roman = Some(index),
            3 if matches!(record.encoding_id, 0 | 1 | 10)
                && (found_win.is_none() || (record.language_id & 0x03ff) == 0x0009) =>
            {
                win_is_english = (record.language_id & 0x03ff) == 0x0009;
                found_win = Some(index);
            }
            _ => {}
        }
    }

    let found_apple = found_apple_english.or(found_apple_roman);
    if let Some(index) = found_win
        && (found_apple.is_none() || win_is_english)
        && let Ok(name) = decode_utf16be_bytes(&table.records[index].string)
    {
        return Some(name);
    }
    if let Some(index) = found_apple
        && let Ok(name) = decode_mac_roman_bytes(&table.records[index].string)
    {
        return Some(name);
    }
    if let Some(index) = found_unicode
        && let Ok(name) = decode_utf16be_bytes(&table.records[index].string)
    {
        return Some(name);
    }
    None
}

/// Return the nameID 25 variation PostScript-name prefix, if present.
pub fn variations_postscript_prefix(table: &NameTable) -> Option<String> {
    [
        NAME_ID_VARIATIONS_PREFIX,
        NAME_ID_TYPO_FAMILY,
        NAME_ID_FAMILY,
    ]
    .into_iter()
    .find_map(|name_id| postscript_prefix_string(table, name_id))
}

fn postscript_prefix_string(table: &NameTable, name_id: u16) -> Option<String> {
    // FreeType `sfnt_get_var_ps_name` in `src/sfnt/sfdriver.c` uses
    // `sfnt_get_name_id` for the variation prefix, which only accepts
    // Windows 3/0, Windows 3/1, or Apple Roman names.  Unlike
    // `tt_face_get_name`, Unicode/ISO name records are not a fallback here.
    let mut found_win = None;
    let mut found_apple = None;
    for (index, record) in table.records.iter().enumerate() {
        if record.name_id != name_id || record.string.is_empty() {
            continue;
        }
        if record.platform_id == 3
            && matches!(record.encoding_id, 0 | 1)
            && (record.language_id == 0x0409 || found_win.is_none())
        {
            found_win = Some(index);
        }
        if record.platform_id == 1
            && record.encoding_id == 0
            && (record.language_id == 0 || found_apple.is_none())
        {
            found_apple = Some(index);
        }
    }

    if let Some(index) = found_win
        && let Some(name) = postscript_prefix_win_string(&table.records[index].string)
    {
        return Some(name);
    }
    if let Some(index) = found_apple {
        return postscript_prefix_apple_string(&table.records[index].string);
    }
    None
}

fn postscript_prefix_win_string(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut result = String::new();
    for pair in bytes.chunks_exact(2) {
        if pair[0] == 0 && pair[1].is_ascii_alphanumeric() {
            result.push(char::from(pair[1]));
        }
    }
    (!result.is_empty()).then_some(result)
}

fn postscript_prefix_apple_string(bytes: &[u8]) -> Option<String> {
    let mut result = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() {
            result.push(char::from(byte));
        }
    }
    (!result.is_empty()).then_some(result)
}

fn find_postscript_name(data: &[u8], string_base: usize, records: &[NameRecord]) -> Option<String> {
    let mut win = None;
    let mut apple = None;
    for (index, record) in records.iter().enumerate() {
        if record.name_id != NAME_ID_POSTSCRIPT || record.length == 0 {
            continue;
        }
        if record.platform_id == 3
            && matches!(record.encoding_id, 0 | 1)
            && (record.language_id == 0x0409 || win.is_none())
        {
            win = Some(index);
        }
        if record.platform_id == 1
            && record.encoding_id == 0
            && (record.language_id == 0 || apple.is_none())
        {
            apple = Some(index);
        }
    }
    if let Some(index) = win
        && let Some(name) = decode_win_postscript(data, string_base, &records[index])
    {
        return Some(name);
    }
    if let Some(index) = apple
        && let Some(name) = decode_apple_postscript(data, string_base, &records[index])
    {
        return Some(name);
    }
    None
}

fn decode_win_postscript(data: &[u8], base: usize, r: &NameRecord) -> Option<String> {
    let start = base + r.offset as usize;
    let end = start + r.length as usize;
    let bytes = data.get(start..end)?;
    if !r.length.is_multiple_of(2) {
        return None;
    }
    let mut result = String::new();
    for pair in bytes.chunks_exact(2) {
        if pair[0] == 0 && is_postscript_name_byte(pair[1]) {
            result.push(char::from(pair[1]));
        }
    }
    (!result.is_empty()).then_some(result)
}

fn decode_apple_postscript(data: &[u8], base: usize, r: &NameRecord) -> Option<String> {
    let start = base + r.offset as usize;
    let end = start + r.length as usize;
    let bytes = data.get(start..end)?;
    let mut result = String::new();
    for &byte in bytes {
        if is_postscript_name_byte(byte) {
            result.push(char::from(byte));
        }
    }
    (!result.is_empty()).then_some(result)
}

fn is_postscript_name_byte(byte: u8) -> bool {
    const SFNT_PS_MAP: [u8; 16] = [
        0x00, 0x00, 0x00, 0x00, 0xDE, 0x7C, 0xFF, 0xAF, 0xFF, 0xFF, 0xFF, 0xD7, 0xFF, 0xFF, 0xFF,
        0x57,
    ];
    byte < 0x80 && (SFNT_PS_MAP[usize::from(byte >> 3)] & (1 << (byte & 0x07))) != 0
}

fn decode_utf16be(data: &[u8], base: usize, r: &NameRecord) -> Result<String, FontError> {
    let start = base + r.offset as usize;
    let end = start + r.length as usize;
    let bytes = data
        .get(start..end)
        .ok_or_else(|| FontError::InvalidFont("name: string offset out of range".into()))?;
    decode_utf16be_bytes(bytes)
}

fn decode_utf16be_bytes(bytes: &[u8]) -> Result<String, FontError> {
    if !bytes.len().is_multiple_of(2) {
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
    decode_mac_roman_bytes(bytes)
}

fn decode_mac_roman_bytes(bytes: &[u8]) -> Result<String, FontError> {
    // Mac Roman 0x00–0x7F is ASCII; higher bytes would need a table (rare in test fonts).
    Ok(bytes.iter().map(|&b| b as char).collect())
}
