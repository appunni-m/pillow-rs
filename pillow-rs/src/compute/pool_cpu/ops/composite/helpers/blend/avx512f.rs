//! AVX-512F dword Blend blocks shared by exact native-byte mode strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[allow(unsafe_code)]
pub(crate) fn execute(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    channels: u8,
    alpha: f64,
) -> Result<DynamicImage, PilError> {
    super::execute_vector_avx512f(
        source,
        other,
        mode,
        channels,
        alpha,
        |left, right, alpha| {
            // SAFETY: this entry is reached only through the guarded AVX-512F strategy.
            unsafe { blend_block(left, right, alpha) }
        },
    )
}

#[allow(unsafe_code)]
#[inline]
#[target_feature(enable = "avx512f")]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
unsafe fn blend_block(left: [u8; 64], right: [u8; 64], alpha: f32) -> [u8; 64] {
    // SAFETY: the prepared AVX-512F strategy guarantees the target feature;
    // these arrays provide exactly 64 initialized input and output bytes.
    unsafe {
        let left = arch::_mm512_loadu_si512(left.as_ptr().cast());
        let right = arch::_mm512_loadu_si512(right.as_ptr().cast());
        let mask = arch::_mm512_set1_epi32(255);
        let alpha = arch::_mm512_set1_ps(alpha);
        let zero = arch::_mm512_setzero_ps();
        let maximum = arch::_mm512_set1_ps(255.0);

        macro_rules! blend_channel {
            ($shift:literal) => {{
                let left = arch::_mm512_and_epi32(arch::_mm512_srli_epi32::<$shift>(left), mask);
                let right = arch::_mm512_and_epi32(arch::_mm512_srli_epi32::<$shift>(right), mask);
                let left = arch::_mm512_cvtepi32_ps(left);
                let right = arch::_mm512_cvtepi32_ps(right);
                let value = arch::_mm512_fmadd_ps(alpha, arch::_mm512_sub_ps(right, left), left);
                let value = arch::_mm512_min_ps(maximum, arch::_mm512_max_ps(zero, value));
                arch::_mm512_slli_epi32::<$shift>(arch::_mm512_cvttps_epi32(value))
            }};
        }

        let channels = arch::_mm512_or_si512(
            arch::_mm512_or_si512(blend_channel!(0), blend_channel!(8)),
            arch::_mm512_or_si512(blend_channel!(16), blend_channel!(24)),
        );
        let mut output = [0u8; 64];
        arch::_mm512_storeu_si512(output.as_mut_ptr().cast(), channels);
        output
    }
}
