//! 'hmtx' table — Horizontal Metrics. Mirrors `tt_face_get_location`-style access.
//!
//! Reference: `src/sfnt/ttload.c`, `tt_face_load_hmtx`.

use crate::error::FontError;

/// Horizontal metrics for a single glyph.
#[derive(Debug, Clone, Copy, Default)]
pub struct LongHorMetric {
    /// Advance width in font design units.
    pub advance_width: u16,
    /// Left side bearing in font design units.
    pub lsb: i16,
}

/// Parsed 'hmtx' table.
#[derive(Debug, Clone, Default)]
pub struct HmtxTable {
    /// Metrics for the first `num_hmetrics` glyphs.
    pub h_metrics: Vec<LongHorMetric>,
    /// Left side bearings for glyphs beyond `num_hmetrics`.
    pub left_side_bearings: Vec<i16>,
}

impl HmtxTable {
    /// Get horizontal metrics for a glyph index. Glyphs past `num_hmetrics`
    /// reuse the last advance width and take their own lsb.
    pub fn get(&self, glyph_index: u16) -> LongHorMetric {
        let idx = glyph_index as usize;
        if idx < self.h_metrics.len() {
            self.h_metrics[idx]
        } else {
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

/// Parse the 'hmtx' table. `num_hmetrics` from hhea, `num_glyphs` from maxp.
pub fn parse_hmtx(data: &[u8], num_hmetrics: u16, num_glyphs: u16) -> Result<HmtxTable, FontError> {
    let hm_count = num_hmetrics as usize;
    let total_glyphs = num_glyphs as usize;

    if hm_count > total_glyphs || hm_count == 0 {
        return Err(FontError::InvalidFont(
            "hmtx: num_hmetrics out of range".into(),
        ));
    }

    let needed = hm_count * 4 + (total_glyphs - hm_count) * 2;
    if data.len() < needed {
        return Err(FontError::InvalidFont(format!(
            "hmtx table too short: need {needed} bytes, have {}",
            data.len()
        )));
    }

    let mut h_metrics = Vec::with_capacity(hm_count);
    for i in 0..hm_count {
        let off = i * 4;
        h_metrics.push(LongHorMetric {
            advance_width: u16::from_be_bytes([data[off], data[off + 1]]),
            lsb: i16::from_be_bytes([data[off + 2], data[off + 3]]),
        });
    }

    let lsb_start = hm_count * 4;
    let lsb_count = total_glyphs - hm_count;
    let mut left_side_bearings = Vec::with_capacity(lsb_count);
    for i in 0..lsb_count {
        let off = lsb_start + i * 2;
        left_side_bearings.push(i16::from_be_bytes([data[off], data[off + 1]]));
    }

    Ok(HmtxTable {
        h_metrics,
        left_side_bearings,
    })
}
