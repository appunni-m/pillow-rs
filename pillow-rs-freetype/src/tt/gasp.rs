//! `gasp` table parsing for `FT_Get_Gasp`.

use crate::error::FontError;

/// FreeType sentinel returned when no usable `gasp` range matches.
pub const FT_GASP_NO_TABLE: i32 = -1;

/// Parsed TrueType/OpenType `gasp` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaspTable {
    version: u16,
    ranges: Vec<GaspRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GaspRange {
    max_ppem: u16,
    flags: u16,
}

impl GaspTable {
    /// Return the FreeType `FT_Get_Gasp` result for a queried ppem.
    pub fn get(&self, ppem: u32) -> i32 {
        let Some(range) = self
            .ranges
            .iter()
            .find(|range| ppem <= u32::from(range.max_ppem))
        else {
            return FT_GASP_NO_TABLE;
        };
        // ftgasp.c masks version 0 tables to legacy bits even if the table
        // carries spurious high bits.
        let flags = if self.version == 0 {
            range.flags & 0x0003
        } else {
            range.flags
        };
        i32::from(flags)
    }
}

/// Parse a SFNT `gasp` table.
///
/// Mirrors `tt_face_load_gasp` in FreeType's `src/sfnt/ttload.c`: versions
/// above 1 and truncated range arrays make the optional table unusable.
pub fn parse_gasp(data: &[u8]) -> Result<GaspTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("gasp table too short".into()));
    }
    let version = read_u16(data, 0);
    if version > 1 {
        return Err(FontError::InvalidFont(format!(
            "unsupported gasp table version {version}"
        )));
    }
    let num_ranges = usize::from(read_u16(data, 2));
    // C `tt_face_load_gasp` enters `num_ranges * 4L` bytes directly.  The
    // source value is a u16, so the complete header and range array are
    // bounded to 262,144 bytes on the supported native and wasm32 targets.
    let ranges_end = 4 + num_ranges * 4;
    let range_bytes = data
        .get(4..ranges_end)
        .ok_or_else(|| FontError::InvalidFont("gasp ranges truncated".into()))?;

    let ranges = range_bytes
        .chunks_exact(4)
        .map(|chunk| GaspRange {
            max_ppem: u16::from_be_bytes([chunk[0], chunk[1]]),
            flags: u16::from_be_bytes([chunk[2], chunk[3]]),
        })
        .collect();

    Ok(GaspTable { version, ranges })
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}
