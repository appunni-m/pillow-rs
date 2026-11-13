//! Checked tap and source-address plans for the I32 NEON Resize strategy.

pub(in crate::compute::pool_cpu::ops::resize::i32) const LANES: usize = 8;

use crate::ops::pil_resize::FilterCoeffsF64;
use wide::f64x8;

#[derive(Clone, Copy)]
pub(super) struct HorizontalTap {
    pub(super) source_indices: [usize; LANES],
    pub(super) active_lanes: u8,
    pub(super) weights: f64x8,
    pub(super) neon_source_start: Option<usize>,
    pub(super) neon_weight: Option<f64>,
}

pub(super) struct HorizontalBlock {
    pub(super) taps: Vec<HorizontalTap>,
    pub(super) neon_eligible: bool,
}

pub(super) struct HorizontalPlan {
    pub(super) blocks: Vec<HorizontalBlock>,
}

/// Resolve checked horizontal tap indices and repeated-lane NEON eligibility.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn build_horizontal(
    coefficients: &FilterCoeffsF64,
    source_width: usize,
    output_width: usize,
) -> Option<HorizontalPlan> {
    let mut blocks = Vec::with_capacity(output_width.div_ceil(LANES));
    for output_x in (0..output_width).step_by(LANES) {
        let lane_count = (output_width - output_x).min(LANES);
        let max_taps = (0..lane_count)
            .filter_map(|lane| coefficients.weights.get(output_x + lane))
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        let mut taps = Vec::with_capacity(max_taps);
        let mut neon_eligible = lane_count == LANES;
        for tap_index in 0..max_taps {
            let mut source_indices = [0usize; LANES];
            let mut weights = [0.0; LANES];
            let mut active_lanes = 0u8;
            for lane in 0..lane_count {
                let output_index = output_x + lane;
                let lane_weights = coefficients.weights.get(output_index)?;
                let Some(&weight) = lane_weights.get(tap_index) else {
                    continue;
                };
                let source_x = usize::try_from(*coefficients.xmin.get(output_index)?)
                    .ok()?
                    .checked_add(tap_index)?;
                if source_x >= source_width {
                    return None;
                }
                source_indices[lane] = source_x;
                weights[lane] = weight;
                active_lanes |= 1 << lane;
            }
            let (neon_source_start, neon_weight) =
                neon_tap_plan(active_lanes, &source_indices, &weights, source_width);
            neon_eligible &= neon_source_start.is_some() && neon_weight.is_some();
            taps.push(HorizontalTap {
                source_indices,
                active_lanes,
                weights: f64x8::new(weights),
                neon_source_start,
                neon_weight,
            });
        }
        blocks.push(HorizontalBlock {
            taps,
            neon_eligible,
        });
    }
    Some(HorizontalPlan { blocks })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn neon_tap_plan(
    active_lanes: u8,
    source_indices: &[usize; LANES],
    weights: &[f64; LANES],
    source_width: usize,
) -> (Option<usize>, Option<f64>) {
    if active_lanes != u8::MAX {
        return (None, None);
    }
    let first_source = source_indices[0];
    let stride_two = source_indices
        .iter()
        .enumerate()
        .all(|(lane, &source)| first_source.checked_add(lane * 2) == Some(source));
    let source_span_fits = first_source
        .checked_add(LANES * 2)
        .is_some_and(|end| end <= source_width);
    let first_weight = weights[0];
    let weights_match = weights
        .iter()
        .all(|weight| weight.to_bits() == first_weight.to_bits());
    (
        (stride_two && source_span_fits).then_some(first_source),
        weights_match.then_some(first_weight),
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[inline]
pub(super) fn horizontal_values(source_row: &[i32], tap: &HorizontalTap) -> [f64; LANES] {
    std::array::from_fn(|lane| {
        if tap.active_lanes & (1 << lane) != 0 {
            f64::from(source_row[tap.source_indices[lane]])
        } else {
            0.0
        }
    })
}
