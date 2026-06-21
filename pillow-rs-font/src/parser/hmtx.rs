//! 'hmtx' table — Horizontal Metrics.
//!
//! Contains advance width and left side bearing for each glyph.

use crate::error::FontError;

/// Horizontal metrics for a single glyph.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LongHorMetric {
    /// Advance width in font design units.
    pub advance_width: u16,
    /// Left side bearing in font design units.
    pub lsb: i16,
}

/// Parsed 'hmtx' table.
#[derive(Debug, Clone)]
pub(crate) struct HmtxTable {
    /// Metrics for the first `num_hmetrics` glyphs (explicit advance width).
    pub h_metrics: Vec<LongHorMetric>,
    /// Left side bearings for remaining glyphs (share last advance width).
    pub left_side_bearings: Vec<i16>,
}

impl HmtxTable {
    /// Get the horizontal metrics for a glyph index.
    pub fn get(&self, glyph_index: u16) -> LongHorMetric {
        let idx = glyph_index as usize;
        if idx < self.h_metrics.len() {
            self.h_metrics[idx]
        } else {
            // Use last advance_width with per-glyph lsb
            let last_advance = self.h_metrics.last().map_or(0, |m| m.advance_width);
            let lsb = self
                .left_side_bearings
                .get(idx - self.h_metrics.len())
                .copied()
                .unwrap_or(0);
            LongHorMetric {
                advance_width: last_advance,
                lsb,
            }
        }
    }
}

/// Parse 'hmtx' table. `num_hmetrics` from hhea, `num_glyphs` from maxp.
pub(crate) fn parse_hmtx(
    data: &[u8],
    num_hmetrics: u16,
    num_glyphs: u16,
) -> Result<HmtxTable, FontError> {
    let hm_count = num_hmetrics as usize;
    let total_glyphs = num_glyphs as usize;

    if hm_count > total_glyphs || hm_count == 0 {
        return Err(FontError::InvalidFont(
            "hmtx: num_hmetrics out of range".into(),
        ));
    }

    let long_entry_size = 4usize; // advance_width(u16) + lsb(i16)
    let needed = hm_count * long_entry_size + (total_glyphs - hm_count) * 2;
    if data.len() < needed {
        return Err(FontError::InvalidFont(format!(
            "hmtx table too short: need {} bytes, have {}",
            needed,
            data.len()
        )));
    }

    let mut h_metrics = Vec::with_capacity(hm_count);
    for i in 0..hm_count {
        let off = i * long_entry_size;
        let advance_width = u16::from_be_bytes([data[off], data[off + 1]]);
        let lsb = i16::from_be_bytes([data[off + 2], data[off + 3]]);
        h_metrics.push(LongHorMetric { advance_width, lsb });
    }

    let lsb_start = hm_count * long_entry_size;
    let lsb_count = total_glyphs - hm_count;
    let mut left_side_bearings = Vec::with_capacity(lsb_count);
    for i in 0..lsb_count {
        let off = lsb_start + i * 2;
        let lsb = i16::from_be_bytes([data[off], data[off + 1]]);
        left_side_bearings.push(lsb);
    }

    Ok(HmtxTable {
        h_metrics,
        left_side_bearings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hmtx_with_single_glyph() {
        let mut data = vec![0u8; 4]; // one long metric
        data[0..2].copy_from_slice(&[0x05, 0x00]); // advance = 1280
        data[2..4].copy_from_slice(&[0x00, 0x20]); // lsb = 32

        let hmtx = parse_hmtx(&data, 1, 1).expect("should parse");
        assert_eq!(hmtx.h_metrics.len(), 1);
        assert_eq!(hmtx.get(0).advance_width, 1280);
        assert_eq!(hmtx.get(0).lsb, 32);
    }

    #[test]
    fn trailing_lsb_for_extra_glyphs() {
        // num_hmetrics=1, num_glyphs=3
        // glyph 0: advance=100, lsb=10
        // glyph 1: advance=100 (reused), lsb=20
        // glyph 2: advance=100 (reused), lsb=30
        let mut data = vec![0u8; 4 + 4]; // one long + two lsb
        data[0..2].copy_from_slice(&[0x00, 0x64]); // glyph 0: advance = 100
        data[2..4].copy_from_slice(&[0x00, 0x0A]); // glyph 0: lsb = 10
        data[4..6].copy_from_slice(&[0x00, 0x14]); // glyph 1: lsb = 20
        data[6..8].copy_from_slice(&[0x00, 0x1E]); // glyph 2: lsb = 30

        let hmtx = parse_hmtx(&data, 1, 3).expect("should parse");
        assert_eq!(hmtx.get(0).advance_width, 100);
        assert_eq!(hmtx.get(0).lsb, 10);
        assert_eq!(hmtx.get(1).advance_width, 100); // reused
        assert_eq!(hmtx.get(1).lsb, 20);
        assert_eq!(hmtx.get(2).advance_width, 100); // reused
        assert_eq!(hmtx.get(2).lsb, 30);
    }
}
