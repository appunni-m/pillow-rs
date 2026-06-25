//! 'name' table — Naming Table.
//!
//! Contains font family name, subfamily (style), and other metadata strings.
//! Prefers platform 3 (Windows), encoding 1 (Unicode BMP = UTF-16BE) per FreeType 2.6.

use crate::error::FontError;

/// Font identification strings extracted from the name table.
#[derive(Debug, Clone)]
pub(crate) struct NameTable {
    /// Font family name (nameID 1).
    pub family: String,
    /// Font subfamily/style name (nameID 2).
    pub subfamily: String,
}

/// Name record entry from the table.
#[derive(Debug)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    /// Language ID (platform-specific).
    _language_id: u16,
    /// Name identifier (1 = family, 2 = subfamily, etc.).
    name_id: u16,
    /// Byte offset within the string storage area.
    offset: u16,
    /// Length in bytes.
    length: u16,
}

/// Parse the 'name' table from raw bytes.
pub(crate) fn parse_name(data: &[u8]) -> Result<NameTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "name table too short (need 6 bytes)".into(),
        ));
    }
    let _format = u16::from_be_bytes([data[0], data[1]]);
    let count = u16::from_be_bytes([data[2], data[3]]) as usize;
    let string_offset = u16::from_be_bytes([data[4], data[5]]) as usize;

    if data.len() < 6 + count * 12 {
        return Err(FontError::InvalidFont(
            "name table: records overflow data".into(),
        ));
    }

    let mut records: Vec<NameRecord> = Vec::with_capacity(count);
    for i in 0..count {
        let off = 6 + i * 12;
        let platform_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let encoding_id = u16::from_be_bytes([data[off + 2], data[off + 3]]);
        let _language_id = u16::from_be_bytes([data[off + 4], data[off + 5]]);
        let name_id = u16::from_be_bytes([data[off + 6], data[off + 7]]);
        let length = u16::from_be_bytes([data[off + 8], data[off + 9]]);
        let str_off = u16::from_be_bytes([data[off + 10], data[off + 11]]);
        records.push(NameRecord {
            platform_id,
            encoding_id,
            _language_id,
            name_id,
            offset: str_off,
            length,
        });
    }

    let family = find_name_string(data, string_offset, &records, 1);
    let subfamily = find_name_string(data, string_offset, &records, 2);

    Ok(NameTable {
        family: family.unwrap_or_else(|| "Unknown".into()),
        subfamily: subfamily.unwrap_or_else(|| "Regular".into()),
    })
}

/// Search for a name string by name_id, preferring platform 3 encoding 1.
fn find_name_string(
    data: &[u8],
    string_base: usize,
    records: &[NameRecord],
    name_id: u16,
) -> Option<String> {
    // Priority 1: platform 3 (Windows), encoding 1 (Unicode BMP)
    for r in records {
        if r.name_id == name_id && r.platform_id == 3 && r.encoding_id == 1 {
            if let Ok(s) = decode_utf16be(data, string_base, r.offset as usize, r.length as usize) {
                return Some(s);
            }
        }
    }
    // Priority 2: platform 1 (Mac), encoding 0 (Roman)
    for r in records {
        if r.name_id == name_id && r.platform_id == 1 && r.encoding_id == 0 {
            if let Ok(s) = decode_mac_roman(data, string_base, r.offset as usize, r.length as usize)
            {
                return Some(s);
            }
        }
    }
    // Fallback: any record with matching name_id
    for r in records {
        if r.name_id == name_id {
            if r.platform_id == 3 {
                if let Ok(s) =
                    decode_utf16be(data, string_base, r.offset as usize, r.length as usize)
                {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Decode a UTF-16BE string from the name table's string storage.
fn decode_utf16be(
    data: &[u8],
    base: usize,
    offset: usize,
    length: usize,
) -> Result<String, FontError> {
    let start = base + offset;
    let end = start + length;
    let bytes = data
        .get(start..end)
        .ok_or_else(|| FontError::InvalidFont("name: string offset out of range".into()))?;
    if length % 2 != 0 {
        return Err(FontError::InvalidFont(
            "name: UTF-16BE string has odd length".into(),
        ));
    }
    let chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&chars)
        .map_err(|e| FontError::InvalidFont(format!("name: invalid UTF-16: {}", e)))
}

/// Decode a Mac Roman string from the name table. Maps bytes 0-127 directly (ASCII subset).
fn decode_mac_roman(
    data: &[u8],
    base: usize,
    offset: usize,
    length: usize,
) -> Result<String, FontError> {
    let start = base + offset;
    let end = start + length;
    let bytes = data.get(start..end).ok_or_else(|| {
        FontError::InvalidFont("name: Mac Roman string offset out of range".into())
    })?;
    // Mac Roman bytes 0x00-0x7F are ASCII. Higher bytes need a mapping table
    // which we skip for now — non-ASCII Mac names are rare in test fonts.
    Ok(bytes.iter().map(|&b| b as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_name_table(names: &[(u16, u16, u16, u16, &str)]) -> Vec<u8> {
        // names: (platform_id, encoding_id, language_id, name_id, text)
        let mut records_bytes = Vec::new();
        let mut strings_bytes = Vec::new();
        let count = names.len() as u16;
        let header_size = 6u16;
        let record_size = 12u16;

        for (pid, eid, lid, nid, text) in names {
            // Encode as UTF-16BE for platform 3
            let mut encoded: Vec<u8> = Vec::new();
            for ch in text.encode_utf16() {
                encoded.extend_from_slice(&ch.to_be_bytes());
            }
            let off = strings_bytes.len() as u16;
            let len = encoded.len() as u16;
            records_bytes.extend_from_slice(&pid.to_be_bytes());
            records_bytes.extend_from_slice(&eid.to_be_bytes());
            records_bytes.extend_from_slice(&lid.to_be_bytes());
            records_bytes.extend_from_slice(&nid.to_be_bytes());
            records_bytes.extend_from_slice(&len.to_be_bytes());
            records_bytes.extend_from_slice(&off.to_be_bytes());
            strings_bytes.extend(&encoded);
        }

        let string_offset = header_size + count * record_size;
        let mut data = Vec::new();
        data.extend_from_slice(&[0x00, 0x00]); // format = 0
        data.extend_from_slice(&count.to_be_bytes());
        data.extend_from_slice(&string_offset.to_be_bytes());
        data.extend(&records_bytes);
        data.extend(&strings_bytes);
        data
    }

    #[test]
    fn extract_family_and_style_platform_3_encoding_1() {
        let data = build_name_table(&[(3, 1, 0x0409, 1, "DejaVu Sans"), (3, 1, 0x0409, 2, "Book")]);
        let name = parse_name(&data).expect("should parse");
        assert_eq!(name.family, "DejaVu Sans");
        assert_eq!(name.subfamily, "Book");
    }

    #[test]
    fn missing_name_yields_unknown_fallback() {
        let data = build_name_table(&[(3, 1, 0x0409, 1, "Test")]); // no nameID 2
        let name = parse_name(&data).expect("should parse");
        assert_eq!(name.family, "Test");
        assert_eq!(name.subfamily, "Regular");
    }
}
