//! 'vmtx' table — Vertical Metrics.
//!
//! Mirrors the same long-metric fallback shape as 'hmtx'.

use crate::error::FontError;

/// Vertical metrics for a single glyph.
#[derive(Debug, Clone, Copy, Default)]
pub struct LongVerMetric {
    /// Advance height in font design units.
    pub advance_height: u16,
    /// Top side bearing in font design units.
    pub tsb: i16,
}

/// Parsed 'vmtx' table.
#[derive(Debug, Clone, Default)]
pub struct VmtxTable {
    /// Metrics for the first `num_vmetrics` glyphs.
    pub v_metrics: Vec<LongVerMetric>,
    /// Top side bearings for glyphs beyond `num_vmetrics`.
    pub top_side_bearings: Vec<i16>,
}

impl VmtxTable {
    /// Get vertical metrics for a glyph index. Glyphs past `num_vmetrics`
    /// reuse the last advance height and take their own top side bearing.
    pub fn get(&self, glyph_index: u16) -> LongVerMetric {
        let idx = glyph_index as usize;
        if idx < self.v_metrics.len() {
            self.v_metrics[idx]
        } else {
            let last_advance = self.v_metrics.last().map_or(0, |m| m.advance_height);
            let tsb = self
                .top_side_bearings
                .get(idx - self.v_metrics.len())
                .copied()
                .unwrap_or(0);
            LongVerMetric {
                advance_height: last_advance,
                tsb,
            }
        }
    }
}

/// Parse the 'vmtx' table. `num_vmetrics` from vhea, `num_glyphs` from maxp.
pub fn parse_vmtx(data: &[u8], num_vmetrics: u16, num_glyphs: u16) -> Result<VmtxTable, FontError> {
    let vm_count = num_vmetrics as usize;
    let total_glyphs = num_glyphs as usize;

    if vm_count > total_glyphs || vm_count == 0 {
        return Err(FontError::InvalidFont(
            "vmtx: num_vmetrics out of range".into(),
        ));
    }

    let needed = vm_count * 4 + (total_glyphs - vm_count) * 2;
    if data.len() < needed {
        return Err(FontError::InvalidFont(format!(
            "vmtx table too short: need {needed} bytes, have {}",
            data.len()
        )));
    }

    let mut v_metrics = Vec::with_capacity(vm_count);
    for i in 0..vm_count {
        let off = i * 4;
        v_metrics.push(LongVerMetric {
            advance_height: u16::from_be_bytes([data[off], data[off + 1]]),
            tsb: i16::from_be_bytes([data[off + 2], data[off + 3]]),
        });
    }

    let tsb_start = vm_count * 4;
    let tsb_count = total_glyphs - vm_count;
    let mut top_side_bearings = Vec::with_capacity(tsb_count);
    for i in 0..tsb_count {
        let off = tsb_start + i * 2;
        top_side_bearings.push(i16::from_be_bytes([data[off], data[off + 1]]));
    }

    Ok(VmtxTable {
        v_metrics,
        top_side_bearings,
    })
}
