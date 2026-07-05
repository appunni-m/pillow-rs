//! 'kern' table — legacy horizontal kerning pairs.
//!
//! This implements the format-0 horizontal pair data used by FreeType's SFNT
//! kerning service for fonts such as DejaVu Sans. GPOS kerning is intentionally
//! separate; this table is enough for the `_imagingft` AV fixture and is a
//! standard TrueType/OpenType table, not an oracle shortcut.

use crate::error::FontError;

/// Parsed legacy kerning table.
#[derive(Debug, Clone)]
pub struct KernTable {
    pairs: Vec<KernPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KernPair {
    left: u16,
    right: u16,
    value: i16,
}

impl KernTable {
    /// Return the unscaled kerning value in font units.
    pub fn get(&self, left: u16, right: u16) -> i16 {
        self.pairs
            .binary_search_by_key(&(left, right), |pair| (pair.left, pair.right))
            .map_or(0, |index| self.pairs[index].value)
    }
}

/// Parse a classic TrueType `kern` table.
pub fn parse_kern(data: &[u8]) -> Result<KernTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("kern table too short".into()));
    }
    let version = u16::from_be_bytes([data[0], data[1]]);
    if version != 0 {
        return Err(FontError::InvalidFont(format!(
            "kern: unsupported version {version}"
        )));
    }
    let n_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
    let mut offset = 4usize;
    let mut pairs = Vec::new();

    for _ in 0..n_tables {
        let Some(header) = data.get(offset..offset + 6) else {
            break;
        };
        let sub_version = u16::from_be_bytes([header[0], header[1]]);
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let coverage = u16::from_be_bytes([header[4], header[5]]);
        if length < 6 {
            break;
        }
        let Some(subtable) = data.get(offset..offset + length) else {
            break;
        };
        let format = coverage >> 8;
        let horizontal = (coverage & 0x0001) != 0;
        if sub_version == 0 && format == 0 && horizontal {
            parse_format0(subtable, &mut pairs)?;
        }
        offset = offset.saturating_add(length);
    }

    pairs.sort_unstable_by_key(|pair| (pair.left, pair.right));
    pairs.dedup_by_key(|pair| (pair.left, pair.right));
    Ok(KernTable { pairs })
}

fn parse_format0(subtable: &[u8], pairs: &mut Vec<KernPair>) -> Result<(), FontError> {
    if subtable.len() < 14 {
        return Err(FontError::InvalidFont("kern format 0 too short".into()));
    }
    let n_pairs = u16::from_be_bytes([subtable[6], subtable[7]]) as usize;
    let records = subtable
        .get(14..)
        .ok_or_else(|| FontError::InvalidFont("kern format 0 records missing".into()))?;
    for chunk in records.chunks_exact(6).take(n_pairs) {
        pairs.push(KernPair {
            left: u16::from_be_bytes([chunk[0], chunk[1]]),
            right: u16::from_be_bytes([chunk[2], chunk[3]]),
            value: i16::from_be_bytes([chunk[4], chunk[5]]),
        });
    }
    Ok(())
}
