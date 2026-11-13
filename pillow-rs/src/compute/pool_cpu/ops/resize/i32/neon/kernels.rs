//! AArch64 NEON kernels for I32 filtered Resize blocks.

use std::arch::aarch64::{
    float64x2_t, int32x2_t, vaddq_f64, vbslq_f64, vcgeq_f64, vcvtq_f64_s64, vcvtq_s64_f64,
    vdupq_n_f64, vfmaq_f64, vget_high_s32, vget_low_s32, vld1q_s32, vld2q_s32, vmovl_s32,
    vqmovn_s64, vst1_s32,
};

use super::LANES;
use super::plan::HorizontalBlock;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[allow(unsafe_code)]
#[inline]
fn round_up_i32x2(value: float64x2_t) -> int32x2_t {
    // SAFETY: Advanced SIMD is mandatory on AArch64. This sequence applies
    // Pillow's sign-aware half adjustment, truncates to i64, then saturates
    // to i32, matching the scalar f64-to-i32 cast for exceptional values.
    unsafe {
        let zero = vdupq_n_f64(0.0);
        let positive_half = vdupq_n_f64(0.5);
        let negative_half = vdupq_n_f64(-0.5);
        let adjustment = vbslq_f64(vcgeq_f64(value, zero), positive_half, negative_half);
        vqmovn_s64(vcvtq_s64_f64(vaddq_f64(value, adjustment)))
    }
}

/// Execute a horizontal tap plan when every lane has stride-two samples and a
/// uniform coefficient.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[allow(unsafe_code)]
pub(super) fn horizontal_block(
    source_row: &[i32],
    block: &HorizontalBlock,
) -> Option<[i32; LANES]> {
    if !block.neon_eligible || block.taps.is_empty() {
        return None;
    }
    for tap in &block.taps {
        let start = tap.neon_source_start?;
        let end = start.checked_add(LANES * 2)?;
        source_row.get(start..end)?;
        tap.neon_weight?;
    }

    // SAFETY: the plan proves eight active lanes, their stride-two indices,
    // identical coefficients, and each checked sixteen-sample load span.
    // The target is AArch64 with NEON, input rows are immutable, and all
    // accumulators are local and independent.
    let lanes = unsafe {
        let zero = vdupq_n_f64(0.0);
        let mut sums = [zero, zero, zero, zero];
        for tap in &block.taps {
            let start = tap.neon_source_start?;
            let weight = tap.neon_weight?;
            let coefficient = vdupq_n_f64(weight);
            let source = source_row.as_ptr().add(start);
            let first = vld2q_s32(source).0;
            let second = vld2q_s32(source.add(8)).0;
            sums[0] = vfmaq_f64(
                sums[0],
                vcvtq_f64_s64(vmovl_s32(vget_low_s32(first))),
                coefficient,
            );
            sums[1] = vfmaq_f64(
                sums[1],
                vcvtq_f64_s64(vmovl_s32(vget_high_s32(first))),
                coefficient,
            );
            sums[2] = vfmaq_f64(
                sums[2],
                vcvtq_f64_s64(vmovl_s32(vget_low_s32(second))),
                coefficient,
            );
            sums[3] = vfmaq_f64(
                sums[3],
                vcvtq_f64_s64(vmovl_s32(vget_high_s32(second))),
                coefficient,
            );
        }
        let mut output = [0i32; LANES];
        for (pair, sum) in output.chunks_exact_mut(2).zip(sums) {
            vst1_s32(pair.as_mut_ptr(), round_up_i32x2(sum));
        }
        output
    };
    Some(lanes)
}

/// Execute the vertical tap span for eight contiguous intermediate samples.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[allow(unsafe_code)]
pub(super) fn vertical_block(
    source: &[i32],
    row_width: usize,
    output_x: usize,
    first_source_y: usize,
    weights: &[f64],
) -> Option<[i32; LANES]> {
    const MAX_TAPS: usize = 16;
    if weights.is_empty() || weights.len() > MAX_TAPS || output_x.checked_add(LANES)? > row_width {
        return None;
    }

    let mut row_starts = [0usize; MAX_TAPS];
    for (tap, _) in weights.iter().enumerate() {
        let source_y = first_source_y.checked_add(tap)?;
        let start = source_y.checked_mul(row_width)?.checked_add(output_x)?;
        source.get(start..start.checked_add(LANES)?)?;
        row_starts[tap] = start;
    }

    // SAFETY: checked row slices establish eight initialized i32 lanes at
    // each address. The two loads fit within the eight-sample span and the
    // architecture guarantees NEON support.
    let lanes = unsafe {
        let zero = vdupq_n_f64(0.0);
        let mut sums = [zero, zero, zero, zero];
        for (tap, &weight) in weights.iter().enumerate() {
            let row = source.as_ptr().add(row_starts[tap]);
            let first = vld1q_s32(row);
            let second = vld1q_s32(row.add(4));
            let coefficient = vdupq_n_f64(weight);
            sums[0] = vfmaq_f64(
                sums[0],
                vcvtq_f64_s64(vmovl_s32(vget_low_s32(first))),
                coefficient,
            );
            sums[1] = vfmaq_f64(
                sums[1],
                vcvtq_f64_s64(vmovl_s32(vget_high_s32(first))),
                coefficient,
            );
            sums[2] = vfmaq_f64(
                sums[2],
                vcvtq_f64_s64(vmovl_s32(vget_low_s32(second))),
                coefficient,
            );
            sums[3] = vfmaq_f64(
                sums[3],
                vcvtq_f64_s64(vmovl_s32(vget_high_s32(second))),
                coefficient,
            );
        }
        let mut output = [0i32; LANES];
        for (pair, sum) in output.chunks_exact_mut(2).zip(sums) {
            vst1_s32(pair.as_mut_ptr(), round_up_i32x2(sum));
        }
        output
    };
    Some(lanes)
}
