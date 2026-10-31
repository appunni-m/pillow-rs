//! x86 FMA3 byte Blend blocks shared by exact native-byte mode strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

/// Runs the archived block and tail policy after the injected context selected AVX/FMA.
#[allow(unsafe_code)]
pub(crate) fn execute(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    channels: u8,
    alpha: f64,
) -> Result<DynamicImage, PilError> {
    super::execute_vector(source, other, mode, channels, alpha, |left, right, alpha| {
        // SAFETY: this entry is reachable only from a prepared AvxFma strategy;
        // its capability requirement checks AVX and FMA before execution.
        unsafe { blend_block(left, right, alpha) }
    })
}

#[allow(unsafe_code)]
#[inline]
#[target_feature(enable = "avx,fma")]
unsafe fn blend_block(left: [u8; 16], right: [u8; 16], alpha: f32) -> [u8; 16] {
    // SAFETY: the target-feature boundary establishes AVX/FMA. The local
    // arrays contain exactly sixteen initialized bytes and are loaded/stored
    // as unaligned vectors within their complete extents.
    unsafe {
        let left = arch::_mm_loadu_si128(left.as_ptr().cast());
        let right = arch::_mm_loadu_si128(right.as_ptr().cast());
        let mask = arch::_mm_set1_epi32(255);
        let alpha = arch::_mm_set1_ps(alpha);
        let zero = arch::_mm_setzero_ps();
        let maximum = arch::_mm_set1_ps(255.0);

        macro_rules! blend_channel {
            ($shift:literal) => {{
                let left = arch::_mm_and_si128(arch::_mm_srli_epi32(left, $shift), mask);
                let right = arch::_mm_and_si128(arch::_mm_srli_epi32(right, $shift), mask);
                let left = arch::_mm_cvtepi32_ps(left);
                let right = arch::_mm_cvtepi32_ps(right);
                let value = arch::_mm_fmadd_ps(alpha, arch::_mm_sub_ps(right, left), left);
                let value = arch::_mm_min_ps(maximum, arch::_mm_max_ps(zero, value));
                arch::_mm_slli_epi32(arch::_mm_cvttps_epi32(value), $shift)
            }};
        }

        let channels = arch::_mm_or_si128(
            arch::_mm_or_si128(
                blend_channel!(0),
                blend_channel!(8),
            ),
            arch::_mm_or_si128(blend_channel!(16), blend_channel!(24)),
        );
        let mut output = [0u8; 16];
        arch::_mm_storeu_si128(output.as_mut_ptr().cast(), channels);
        output
    }
}
