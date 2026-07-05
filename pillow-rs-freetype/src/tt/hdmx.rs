//! 'hdmx' table — horizontal device metrics.
//!
//! FreeType uses this optional table as a pixel-size-specific horizontal
//! advance override for hinted TrueType loads (`ttgload.c:2299-2313`,
//! `ttgload.c:1974-1977`).

use crate::error::FontError;

#[derive(Debug, Clone)]
pub struct HdmxTable {
    records: Vec<HdmxRecord>,
}

#[derive(Debug, Clone)]
struct HdmxRecord {
    ppem: u8,
    widths: Vec<u8>,
}

impl HdmxTable {
    pub fn width_for_ppem(&self, ppem: i32, glyph_index: u16) -> Option<u8> {
        let ppem = u8::try_from(ppem).ok()?;
        let record = self
            .records
            .binary_search_by_key(&ppem, |record| record.ppem)
            .ok()
            .and_then(|index| self.records.get(index))?;
        record.widths.get(glyph_index as usize).copied()
    }
}

pub fn parse_hdmx(data: &[u8], num_glyphs: u16) -> Result<HdmxTable, FontError> {
    if data.len() < 8 {
        return Err(FontError::InvalidFont("hdmx table too short".into()));
    }

    let num_records = u16::from_be_bytes([data[2], data[3]]) as usize;
    let mut record_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if record_size >= 0xFFFF_0000 {
        record_size &= 0xFFFF;
    }

    if num_records == 0 || num_records > 255 {
        return Err(FontError::InvalidFont(
            "hdmx record count out of range".into(),
        ));
    }

    let expected_record_size = (u32::from(num_glyphs) + 2 + 3) & !3;
    if record_size != expected_record_size {
        return Err(FontError::InvalidFont("hdmx record size mismatch".into()));
    }

    let record_size = record_size as usize;
    let num_glyphs = num_glyphs as usize;
    let mut records = Vec::with_capacity(num_records);
    let mut offset = 8usize;
    for _ in 0..num_records {
        let Some(record) = data.get(offset..offset + record_size) else {
            break;
        };
        records.push(HdmxRecord {
            ppem: record[0],
            widths: record[2..2 + num_glyphs].to_vec(),
        });
        offset += record_size;
    }

    records.sort_by_key(|record| record.ppem);
    Ok(HdmxTable { records })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_finds_width_record() {
        let data = [
            0, 0, // version
            0, 1, // one record
            0, 0, 0, 8, // record size = (3 glyphs + 2 + padding)
            10, 7, // ppem, max width
            4, 5, 6, // glyph widths
            0, 0, 0, // padding
        ];
        let table = parse_hdmx(&data, 3).unwrap_or_else(|err| {
            panic!("valid hdmx parses successfully: {err}");
        });
        assert_eq!(table.width_for_ppem(10, 1), Some(5));
        assert_eq!(table.width_for_ppem(9, 1), None);
    }
}
