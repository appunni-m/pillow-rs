//! AVX2 blocks for four-channel fractional box blur.

#![allow(unsafe_code)]

use core::arch::x86_64::{
    __m256i, _mm256_add_epi32, _mm256_and_si256, _mm256_extract_epi32, _mm256_loadu_si256,
    _mm256_mullo_epi32, _mm256_or_si256, _mm256_permutevar8x32_epi32, _mm256_set_epi32,
    _mm256_set1_epi32, _mm256_slli_epi32, _mm256_slli_si256, _mm256_srli_epi32,
    _mm256_storeu_si256, _mm256_sub_epi32,
};

use super::SIMD_BOX_BLUR_BIAS;

#[target_feature(enable = "avx2")]
pub(super) unsafe fn blur_middle_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; 4],
    whole_weight: u32,
    fractional_weight: u32,
    output_start: usize,
) {
    const CHANNELS: usize = 4;
    const RADIUS: usize = 7;
    const BLOCK_PIXELS: usize = 8;
    let line_length = source.len() / CHANNELS;
    debug_assert_eq!(source.len() % CHANNELS, 0);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert!(output_start >= RADIUS + 1);
    debug_assert!(output_start + BLOCK_PIXELS + RADIUS < line_length);
    debug_assert_ne!(fractional_weight, 0);

    // SAFETY: the full middle-block bounds above establish three readable
    // eight-pixel streams and one writable eight-pixel output span.
    unsafe {
        let leaving_start = (output_start - RADIUS - 1) * CHANNELS;
        let entering_start = (output_start + RADIUS) * CHANNELS;
        let far_right_start = (output_start + RADIUS + 1) * CHANNELS;
        let leaving = _mm256_loadu_si256(source.as_ptr().add(leaving_start).cast());
        let entering = _mm256_loadu_si256(source.as_ptr().add(entering_start).cast());
        let far_right = _mm256_loadu_si256(source.as_ptr().add(far_right_start).cast());

        let (red, red_accumulator) = process_channel::<0>(
            leaving,
            entering,
            far_right,
            accumulator[0],
            whole_weight,
            fractional_weight,
        );
        let (green, green_accumulator) = process_channel::<8>(
            leaving,
            entering,
            far_right,
            accumulator[1],
            whole_weight,
            fractional_weight,
        );
        let (blue, blue_accumulator) = process_channel::<16>(
            leaving,
            entering,
            far_right,
            accumulator[2],
            whole_weight,
            fractional_weight,
        );
        let (alpha, alpha_accumulator) = process_channel::<24>(
            leaving,
            entering,
            far_right,
            accumulator[3],
            whole_weight,
            fractional_weight,
        );

        accumulator[0] = red_accumulator;
        accumulator[1] = green_accumulator;
        accumulator[2] = blue_accumulator;
        accumulator[3] = alpha_accumulator;

        let packed = _mm256_or_si256(
            _mm256_or_si256(red, _mm256_slli_epi32::<8>(green)),
            _mm256_or_si256(
                _mm256_slli_epi32::<16>(blue),
                _mm256_slli_epi32::<24>(alpha),
            ),
        );
        _mm256_storeu_si256(
            destination.as_mut_ptr().add(output_start * CHANNELS).cast(),
            packed,
        );
    }
}

#[target_feature(enable = "avx2")]
unsafe fn process_channel<const SHIFT: i32>(
    leaving_pixels: __m256i,
    entering_pixels: __m256i,
    far_right_pixels: __m256i,
    accumulator: u32,
    whole_weight: u32,
    fractional_weight: u32,
) -> (__m256i, u32) {
    let mask = _mm256_set1_epi32(0xff);
    let leaving = match SHIFT {
        0 => leaving_pixels,
        8 => _mm256_srli_epi32::<8>(leaving_pixels),
        16 => _mm256_srli_epi32::<16>(leaving_pixels),
        24 => _mm256_srli_epi32::<24>(leaving_pixels),
        _ => unreachable!(),
    };
    let entering = match SHIFT {
        0 => entering_pixels,
        8 => _mm256_srli_epi32::<8>(entering_pixels),
        16 => _mm256_srli_epi32::<16>(entering_pixels),
        24 => _mm256_srli_epi32::<24>(entering_pixels),
        _ => unreachable!(),
    };
    let far_right = match SHIFT {
        0 => far_right_pixels,
        8 => _mm256_srli_epi32::<8>(far_right_pixels),
        16 => _mm256_srli_epi32::<16>(far_right_pixels),
        24 => _mm256_srli_epi32::<24>(far_right_pixels),
        _ => unreachable!(),
    };
    let leaving = _mm256_and_si256(leaving, mask);
    let entering = _mm256_and_si256(entering, mask);
    let far_right = _mm256_and_si256(far_right, mask);

    let delta = _mm256_sub_epi32(entering, leaving);
    let partial = _mm256_add_epi32(delta, _mm256_slli_si256::<4>(delta));
    let partial = _mm256_add_epi32(partial, _mm256_slli_si256::<8>(partial));
    let low_total = _mm256_permutevar8x32_epi32(partial, _mm256_set1_epi32(3));
    let high_half = _mm256_set_epi32(-1, -1, -1, -1, 0, 0, 0, 0);
    let prefix = _mm256_add_epi32(partial, _mm256_and_si256(low_total, high_half));
    let accumulated = _mm256_add_epi32(prefix, _mm256_set1_epi32(accumulator as i32));
    let far_sum = _mm256_add_epi32(leaving, far_right);
    let fractional = _mm256_mullo_epi32(far_sum, _mm256_set1_epi32(fractional_weight as i32));
    let bulk = _mm256_add_epi32(
        _mm256_add_epi32(
            _mm256_mullo_epi32(accumulated, _mm256_set1_epi32(whole_weight as i32)),
            fractional,
        ),
        _mm256_set1_epi32(SIMD_BOX_BLUR_BIAS as i32),
    );
    let result = _mm256_srli_epi32::<24>(bulk);
    let final_accumulator = accumulator.wrapping_add(_mm256_extract_epi32::<7>(prefix) as u32);
    (result, final_accumulator)
}

#[cfg(test)]
mod tests {
    use super::super::simd_blur_line_impl;

    const WHOLE_WEIGHT: u32 = 1_116_425;
    const FRACTIONAL_WEIGHT: u32 = 15_420;
    const RADIUS: usize = 7;

    #[test]
    fn rgba_radius_seven_avx2_prefix_blocks_match_scalar_edges_and_tails() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }

        for width in [1, 7, 15, 16, 17, 23, 24, 25, 31, 32, 33, 64, 1024, 1025] {
            for seed in 0..5 {
                let source: Vec<u8> = (0..width * 4)
                    .map(|index| {
                        ((index * 73 + width * 31 + seed * 97 + (index >> 2) * 29) % 256) as u8
                    })
                    .collect();
                let mut expected = vec![0; source.len()];
                let mut actual = vec![0; source.len()];
                simd_blur_line_impl::<false, false>(
                    &source,
                    &mut expected,
                    width,
                    4,
                    RADIUS,
                    WHOLE_WEIGHT,
                    FRACTIONAL_WEIGHT,
                );
                simd_blur_line_impl::<true, true>(
                    &source,
                    &mut actual,
                    width,
                    4,
                    RADIUS,
                    WHOLE_WEIGHT,
                    FRACTIONAL_WEIGHT,
                );
                assert_eq!(actual, expected, "width={width}, seed={seed}");
            }
        }
    }
}
