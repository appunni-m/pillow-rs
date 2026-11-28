//! RGBA radius-seven GaussianBlur AVX-512F middle-block kernel.

#![allow(unsafe_code)]

use core::arch::x86_64::{
    __m512i, _mm512_add_epi32, _mm512_and_si512, _mm512_loadu_si512, _mm512_mask_add_epi32,
    _mm512_mullo_epi32, _mm512_or_si512, _mm512_permutexvar_epi32, _mm512_set1_epi32,
    _mm512_slli_epi32, _mm512_srli_epi32, _mm512_storeu_si512, _mm512_sub_epi32,
};

const CHANNELS: usize = 4;
const RADIUS: usize = 7;
const BLOCK_PIXELS: usize = 16;
const BOX_BLUR_BIAS: u32 = crate::compute::pool_cpu::ops::filter::helpers::BOX_BLUR_BIAS;

const PREFIX_INDEX_1: [i32; BLOCK_PIXELS] = [0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
const PREFIX_INDEX_2: [i32; BLOCK_PIXELS] = [0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const PREFIX_INDEX_4: [i32; BLOCK_PIXELS] = [0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const PREFIX_INDEX_8: [i32; BLOCK_PIXELS] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7];

pub(super) struct Avx512FKernel;

impl super::super::helpers::RadiusSevenHorizontalKernel for Avx512FKernel {
    const HAS_OPTIMIZED_MIDDLE_BLOCK: bool = true;
    const BLOCK_PIXELS: usize = 16;

    fn blur_middle_block(
        source: &[u8],
        destination: &mut [u8],
        accumulator: &mut [u32; CHANNELS],
        whole_weight: u32,
        fractional_weight: u32,
        output_start: usize,
    ) {
        // SAFETY: dispatch selects this kernel only after the captured host's AVX-512F check.
        unsafe {
            blur_middle_block(
                source,
                destination,
                accumulator,
                whole_weight,
                fractional_weight,
                output_start,
            );
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn blur_middle_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    whole_weight: u32,
    fractional_weight: u32,
    output_start: usize,
) {
    let line_length = source.len() / CHANNELS;
    debug_assert_eq!(source.len() % CHANNELS, 0);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert!(output_start > RADIUS);
    debug_assert!(output_start + BLOCK_PIXELS + RADIUS < line_length);
    debug_assert_ne!(fractional_weight, 0);

    // SAFETY: the block predicate proves all three 16-pixel reads and the output store are in-row.
    unsafe {
        let leaving_start = (output_start - RADIUS - 1) * CHANNELS;
        let entering_start = (output_start + RADIUS) * CHANNELS;
        let far_right_start = (output_start + RADIUS + 1) * CHANNELS;
        let leaving = _mm512_loadu_si512(source.as_ptr().add(leaving_start).cast());
        let entering = _mm512_loadu_si512(source.as_ptr().add(entering_start).cast());
        let far_right = _mm512_loadu_si512(source.as_ptr().add(far_right_start).cast());
        let controls = { PrefixControls::load() };
        let red = process_channel::<0>(
            leaving,
            entering,
            far_right,
            accumulator[0],
            whole_weight,
            fractional_weight,
            controls,
        );
        let green = process_channel::<8>(
            leaving,
            entering,
            far_right,
            accumulator[1],
            whole_weight,
            fractional_weight,
            controls,
        );
        let blue = process_channel::<16>(
            leaving,
            entering,
            far_right,
            accumulator[2],
            whole_weight,
            fractional_weight,
            controls,
        );
        let alpha = process_channel::<24>(
            leaving,
            entering,
            far_right,
            accumulator[3],
            whole_weight,
            fractional_weight,
            controls,
        );
        accumulator[0] = red.1;
        accumulator[1] = green.1;
        accumulator[2] = blue.1;
        accumulator[3] = alpha.1;
        let packed = _mm512_or_si512(
            _mm512_or_si512(red.0, _mm512_slli_epi32::<8>(green.0)),
            _mm512_or_si512(
                _mm512_slli_epi32::<16>(blue.0),
                _mm512_slli_epi32::<24>(alpha.0),
            ),
        );
        _mm512_storeu_si512(
            destination.as_mut_ptr().add(output_start * CHANNELS).cast(),
            packed,
        );
    }
}

#[derive(Clone, Copy)]
struct PrefixControls {
    index_1: __m512i,
    index_2: __m512i,
    index_4: __m512i,
    index_8: __m512i,
}

impl PrefixControls {
    #[target_feature(enable = "avx512f")]
    #[inline]
    unsafe fn load() -> Self {
        // SAFETY: each array has sixteen readable i32 lanes; this function is target-feature gated.
        unsafe {
            Self {
                index_1: _mm512_loadu_si512(PREFIX_INDEX_1.as_ptr().cast()),
                index_2: _mm512_loadu_si512(PREFIX_INDEX_2.as_ptr().cast()),
                index_4: _mm512_loadu_si512(PREFIX_INDEX_4.as_ptr().cast()),
                index_8: _mm512_loadu_si512(PREFIX_INDEX_8.as_ptr().cast()),
            }
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn process_channel<const SHIFT: i32>(
    leaving_pixels: __m512i,
    entering_pixels: __m512i,
    far_right_pixels: __m512i,
    accumulator: u32,
    whole_weight: u32,
    fractional_weight: u32,
    controls: PrefixControls,
) -> (__m512i, u32) {
    // SAFETY: this leaf is called only from the AVX-512F middle-block kernel.
    unsafe {
        let mask = _mm512_set1_epi32(0xff);
        let leaving = channel::<SHIFT>(leaving_pixels, mask);
        let entering = channel::<SHIFT>(entering_pixels, mask);
        let far_right = channel::<SHIFT>(far_right_pixels, mask);
        let delta = _mm512_sub_epi32(entering, leaving);
        let prefix = prefix_sum(delta, controls);
        let accumulated = _mm512_add_epi32(prefix, _mm512_set1_epi32(accumulator as i32));
        let far_sum = _mm512_add_epi32(leaving, far_right);
        let fractional = _mm512_mullo_epi32(far_sum, _mm512_set1_epi32(fractional_weight as i32));
        let bulk = _mm512_add_epi32(
            _mm512_add_epi32(
                _mm512_mullo_epi32(accumulated, _mm512_set1_epi32(whole_weight as i32)),
                fractional,
            ),
            _mm512_set1_epi32(BOX_BLUR_BIAS as i32),
        );
        let result = _mm512_srli_epi32::<24>(bulk);
        let mut prefix_lanes = [0i32; BLOCK_PIXELS];
        _mm512_storeu_si512(prefix_lanes.as_mut_ptr().cast(), prefix);
        (
            result,
            accumulator.wrapping_add(prefix_lanes[BLOCK_PIXELS - 1] as u32),
        )
    }
}

#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn channel<const SHIFT: i32>(pixels: __m512i, mask: __m512i) -> __m512i {
    // SAFETY: this leaf is called only from the AVX-512F channel processor.
    {
        let shifted = match SHIFT {
            0 => pixels,
            8 => _mm512_srli_epi32::<8>(pixels),
            16 => _mm512_srli_epi32::<16>(pixels),
            24 => _mm512_srli_epi32::<24>(pixels),
            _ => unreachable!(),
        };
        _mm512_and_si512(shifted, mask)
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn prefix_sum(values: __m512i, controls: PrefixControls) -> __m512i {
    // SAFETY: the caller enters from the AVX-512F channel leaf.
    {
        let mut sum = values;
        sum = _mm512_mask_add_epi32(
            sum,
            0xfffe,
            sum,
            _mm512_permutexvar_epi32(controls.index_1, sum),
        );
        sum = _mm512_mask_add_epi32(
            sum,
            0xfffc,
            sum,
            _mm512_permutexvar_epi32(controls.index_2, sum),
        );
        sum = _mm512_mask_add_epi32(
            sum,
            0xfff0,
            sum,
            _mm512_permutexvar_epi32(controls.index_4, sum),
        );
        _mm512_mask_add_epi32(
            sum,
            0xff00,
            sum,
            _mm512_permutexvar_epi32(controls.index_8, sum),
        )
    }
}
