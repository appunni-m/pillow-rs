//! AVX-512F 2x2 quartet sums shared by I32 Reduce and Thumbnail.

#![allow(unsafe_code)] // Callers admit AVX-512F and complete 16-pixel source blocks.

use crate::compute::helpers::resampling::rounding::round_up;

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

pub(crate) const LANES: usize = 16;

const EVEN_INDICES: [i32; LANES] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30];
const ODD_INDICES: [i32; LANES] = [1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31];

/// # Safety
/// `top` and `bottom` each point to at least 32 initialized `i32` samples, and
/// AVX-512F is enabled by the caller's selected strategy.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
pub(crate) unsafe fn reduce_quartet_block(top: *const i32, bottom: *const i32) -> [i32; LANES] {
    let mut sums = [0i32; LANES];
    // SAFETY: the caller guarantees 32 initialized samples in both rows.
    unsafe {
        let even = arch::_mm512_loadu_si512(EVEN_INDICES.as_ptr().cast());
        let odd = arch::_mm512_loadu_si512(ODD_INDICES.as_ptr().cast());
        let top_first = arch::_mm512_loadu_si512(top.cast());
        let top_second = arch::_mm512_loadu_si512(top.add(LANES).cast());
        let bottom_first = arch::_mm512_loadu_si512(bottom.cast());
        let bottom_second = arch::_mm512_loadu_si512(bottom.add(LANES).cast());
        let top_left = arch::_mm512_permutex2var_epi32(top_first, even, top_second);
        let top_right = arch::_mm512_permutex2var_epi32(top_first, odd, top_second);
        let bottom_left = arch::_mm512_permutex2var_epi32(bottom_first, even, bottom_second);
        let bottom_right = arch::_mm512_permutex2var_epi32(bottom_first, odd, bottom_second);
        let quartets = arch::_mm512_add_epi32(
            arch::_mm512_add_epi32(arch::_mm512_add_epi32(top_left, top_right), bottom_left),
            bottom_right,
        );
        arch::_mm512_storeu_si512(sums.as_mut_ptr().cast(), quartets);
    }
    sums.map(|sum| round_up(f64::from(sum) / 4.0) as i32)
}
