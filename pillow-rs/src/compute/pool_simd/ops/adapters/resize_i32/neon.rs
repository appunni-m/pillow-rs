//! AArch64 NEON kernels for regular native-I resize blocks.

use super::{ResizeIHorizontalBlockPlan, SIMD_RESIZE_LANES};
use crate::ops::pil_resize::round_up;
use std::arch::aarch64::{
    vcvtq_f64_s64, vdupq_n_f64, vfmaq_f64, vget_high_s32, vget_low_s32, vld1q_s32, vld2q_s32,
    vmovl_s32, vst1q_f64,
};

/// Apply the horizontal tap plan when each lane reads a stride-two source
/// sample and every lane shares the same coefficient.
#[allow(unsafe_code)]
pub(super) fn horizontal_block(
    source_row: &[i32],
    block: &ResizeIHorizontalBlockPlan,
) -> Option<[i32; SIMD_RESIZE_LANES]> {
    if !block.neon_eligible || block.taps.is_empty() {
        return None;
    }
    for tap in &block.taps {
        let start = tap.neon_source_start?;
        let end = start.checked_add(SIMD_RESIZE_LANES * 2)?;
        source_row.get(start..end)?;
        tap.neon_weight?;
    }

    // SAFETY: the plan builder sets `neon_eligible` only for eight active
    // lanes with identical weights and source indices at first + 2 * lane.
    // The checked slices above prove that the two interleaved loads together
    // read 16 initialized i32 values from this immutable row. NEON is enabled
    // by the module's compile-time target_feature gate; the four accumulators
    // are independent output lanes and preserve the original tap order.
    let rounded = unsafe {
        let zero = vdupq_n_f64(0.0);
        let mut sums = [zero, zero, zero, zero];
        for tap in &block.taps {
            let start = tap
                .neon_source_start
                .expect("eligible NEON resize tap has a stride-two source span");
            let weight = tap
                .neon_weight
                .expect("eligible NEON resize tap has a uniform coefficient");
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

        let mut lanes = [[0.0f64; 2]; 4];
        for (sum, lane_pair) in sums.into_iter().zip(&mut lanes) {
            vst1q_f64(lane_pair.as_mut_ptr(), sum);
        }
        lanes
    };

    Some(std::array::from_fn(|lane| {
        round_up(rounded[lane / 2][lane % 2]) as i32
    }))
}

/// Apply the vertical tap span to eight contiguous I-mode samples.
#[allow(unsafe_code)]
pub(super) fn vertical_block(
    source: &[i32],
    row_width: usize,
    output_x: usize,
    first_source_y: usize,
    weights: &[f64],
) -> Option<[i32; SIMD_RESIZE_LANES]> {
    const MAX_TAPS: usize = 16;

    if weights.is_empty()
        || weights.len() > MAX_TAPS
        || output_x.checked_add(SIMD_RESIZE_LANES)? > row_width
    {
        return None;
    }

    let mut row_starts = [0usize; MAX_TAPS];
    for (tap, _) in weights.iter().enumerate() {
        let source_y = first_source_y.checked_add(tap)?;
        let start = source_y.checked_mul(row_width)?.checked_add(output_x)?;
        let end = start.checked_add(SIMD_RESIZE_LANES)?;
        source.get(start..end)?;
        row_starts[tap] = start;
    }

    // SAFETY: the checked source slices above prove each row start addresses
    // eight initialized i32 values. The two 4-lane loads stay within each
    // eight-sample slice. The input is immutable, accumulators are local, and
    // AArch64 NEON is enabled by the module's compile-time target_feature gate.
    let rounded = unsafe {
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

        let mut lanes = [[0.0f64; 2]; 4];
        for (sum, lane_pair) in sums.into_iter().zip(&mut lanes) {
            vst1q_f64(lane_pair.as_mut_ptr(), sum);
        }
        lanes
    };

    Some(std::array::from_fn(|lane| {
        round_up(rounded[lane / 2][lane % 2]) as i32
    }))
}

#[cfg(test)]
mod tests {
    use super::{SIMD_RESIZE_LANES, horizontal_block, vertical_block};
    use crate::compute::pool_cpu::ops::geometry::resample_kernel;
    use crate::ops::pil_resize::{precompute_coeffs_f64, round_up};
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};
    use wide::{f64x8, i32x8};

    fn signed_samples(length: usize) -> Vec<i32> {
        (0..length)
            .map(|index| match index % 23 {
                0 => i32::MIN,
                1 => i32::MAX,
                _ => (index as i32).wrapping_mul(31_337).wrapping_sub(2_000_000),
            })
            .collect()
    }

    #[test]
    fn horizontal_neon_stride_two_matches_ordered_wide_fma_for_signed_extremes() {
        let source_width = 128;
        let output_width = 64;
        let (kernel, support) = resample_kernel(&ResampleFilter::Bicubic);
        let coefficients =
            precompute_coeffs_f64(output_width as u32, source_width as u32, kernel, support);
        let plan = super::super::build_resize_i_horizontal_plan_with_neon(
            &coefficients,
            source_width,
            output_width,
        )
        .expect("valid 2x resize coefficients must form a SIMD plan");
        let source = signed_samples(source_width);
        let mut checked_blocks = 0;

        for block in &plan.blocks {
            if !block.neon_eligible {
                continue;
            }
            let mut sums = f64x8::splat(0.0);
            for tap in &block.taps {
                sums = tap.weights.mul_add(
                    f64x8::new(super::super::resize_i_horizontal_values(&source, tap)),
                    sums,
                );
            }
            let expected =
                i32x8::new(sums.to_array().map(|value| round_up(value) as i32)).to_array();
            assert_eq!(horizontal_block(&source, block), Some(expected));
            checked_blocks += 1;
        }

        assert!(checked_blocks > 0, "the 2x interior should use NEON loads");
    }

    #[test]
    fn vertical_neon_matches_ordered_wide_fma_for_signed_extremes() {
        let width = 64;
        let source_height = 128;
        let output_height = 64;
        let (kernel, support) = resample_kernel(&ResampleFilter::Bicubic);
        let coefficients =
            precompute_coeffs_f64(output_height as u32, source_height as u32, kernel, support);
        let source = signed_samples(width * source_height);

        for output_y in 0..output_height {
            let first_source_y = usize::try_from(coefficients.xmin[output_y])
                .expect("resample row start must be nonnegative");
            let weights = &coefficients.weights[output_y];
            for output_x in (0..width).step_by(SIMD_RESIZE_LANES) {
                let mut sums = f64x8::splat(0.0);
                for (tap, &weight) in weights.iter().enumerate() {
                    let source_y = first_source_y + tap;
                    let row_start = source_y * width + output_x;
                    let values = std::array::from_fn(|lane| f64::from(source[row_start + lane]));
                    sums = f64x8::splat(weight).mul_add(f64x8::new(values), sums);
                }
                let expected =
                    i32x8::new(sums.to_array().map(|value| round_up(value) as i32)).to_array();
                assert_eq!(
                    vertical_block(&source, width, output_x, first_source_y, weights),
                    Some(expected)
                );
            }
        }
    }

    #[test]
    fn direct_i_resize_reports_the_neon_kernel_path() {
        use crate::compute::Backend;

        let source = signed_samples(128 * 128)
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(128, 128, source).expect("source shape must match its bytes"),
        );
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        crate::compute::begin_pipeline_operation_telemetry("Resize");
        let result = super::super::simd_resize_i32(&image, 64, 64, &ResampleFilter::Bicubic);
        crate::compute::finish_pipeline_operation_telemetry();
        let records = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        result.expect("I-mode SIMD resize must complete");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].path, "neon-f64-resize");
        assert!(records[0].vector_block_count > 0);
    }
}
