//! AArch64 NEON specialization for native RGB Eval LUTs.
//!
//! The shared portable-wide LUT kernel remains the fallback for non-affine
//! tables and for builds where this architecture strategy is unavailable.

use crate::compute::pool_cpu::ops::eval::helpers::vector::{
    self, Kernel, Mapped, Output,
};
use crate::error::PilError;

/// Execute the RGB NEON path selected by the exact mode strategy.
pub(super) fn execute(kernel: Kernel<'_, 3>) -> Result<Output, PilError> {
    if kernel.is_identity() {
        return vector::execute_neon(kernel);
    }
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    {
        let mapped = if let Some(coefficients) = neon_affine_coefficients(kernel.lut()) {
            neon_rgb_affine_map(&kernel, coefficients)
        } else {
            neon_rgb_lut_map(&kernel)
        };
        if let Some(mapped) = mapped {
            return vector::finish(mapped);
        }
    }
    vector::execute_neon(kernel)
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
fn neon_affine_coefficients(lut: &[u8]) -> Option<[(u8, u8); 3]> {
    if lut.len() != 3 * 256 {
        return None;
    }
    Some([
        crate::compute::byte_lut_affine_coefficients(&lut[0..256])?,
        crate::compute::byte_lut_affine_coefficients(&lut[256..512])?,
        crate::compute::byte_lut_affine_coefficients(&lut[512..768])?,
    ])
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
#[allow(unsafe_code)]
fn neon_rgb_lut_map(kernel: &Kernel<'_, 3>) -> Option<Mapped> {
    use core::arch::aarch64 as neon;

    const CHANNELS_RGB: usize = 3;
    const LANES: usize = 16;
    let row_stride = (kernel.width() as usize).checked_mul(CHANNELS_RGB)?;
    let expected_len = row_stride.checked_mul(kernel.height() as usize)?;
    if row_stride == 0 || kernel.source().len() != expected_len {
        return None;
    }
    // SAFETY: preparation validates three complete 256-entry LUTs. Each
    // offset selects a complete 64-byte bank within the borrowed LUT.
    let tables: [[neon::uint8x16x4_t; 4]; CHANNELS_RGB] = unsafe {
        std::array::from_fn(|channel| {
            std::array::from_fn(|bank| {
                let offset = channel * 256 + bank * 64;
                neon::vld1q_u8_x4(kernel.lut().as_ptr().add(offset))
            })
        })
    };
    #[inline(always)]
    fn lookup(
        input: core::arch::aarch64::uint8x16_t,
        tables: &[core::arch::aarch64::uint8x16x4_t; 4],
    ) -> core::arch::aarch64::uint8x16_t {
        use core::arch::aarch64 as neon;
        // SAFETY: input and lookup tables are valid NEON values, and the
        // masked indices are always in the 0..64 range covered by each bank.
        unsafe {
            let indices = neon::vandq_u8(input, neon::vdupq_n_u8(0x3f));
            let high = neon::vshrq_n_u8(input, 6);
            let low_bank = neon::vceqq_u8(
                neon::vandq_u8(high, neon::vdupq_n_u8(1)),
                neon::vdupq_n_u8(0),
            );
            let high_bank = neon::vceqq_u8(
                neon::vandq_u8(high, neon::vdupq_n_u8(2)),
                neon::vdupq_n_u8(0),
            );
            let bank0 = neon::vqtbl4q_u8(tables[0], indices);
            let bank1 = neon::vqtbl4q_u8(tables[1], indices);
            let bank2 = neon::vqtbl4q_u8(tables[2], indices);
            let bank3 = neon::vqtbl4q_u8(tables[3], indices);
            let low_pair = neon::vbslq_u8(low_bank, bank0, bank1);
            let high_pair = neon::vbslq_u8(low_bank, bank2, bank3);
            neon::vbslq_u8(high_bank, low_pair, high_pair)
        }
    }
    let width = kernel.width() as usize;
    let height = kernel.height() as usize;
    let vector_width = width / LANES * LANES;
    let mut bytes = vec![0u8; expected_len];
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &kernel.source()[row_start..row_start + row_stride];
        let destination_row = &mut bytes[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS_RGB;
            // SAFETY: vector_width is rounded down to complete groups of
            // sixteen RGB pixels, so both 48-byte accesses stay in this row.
            // The source and destination slices are disjoint and NEON is
            // enabled by this function's compile-time target gate.
            unsafe {
                let rgb = neon::vld3q_u8(source_row.as_ptr().add(byte_offset));
                let mapped = neon::uint8x16x3_t(
                    lookup(rgb.0, &tables[0]),
                    lookup(rgb.1, &tables[1]),
                    lookup(rgb.2, &tables[2]),
                );
                neon::vst3q_u8(destination_row.as_mut_ptr().add(byte_offset), mapped);
            }
        }
        for pixel in vector_width..width {
            let byte_offset = pixel * CHANNELS_RGB;
            for channel in 0..CHANNELS_RGB {
                let sample = usize::from(source_row[byte_offset + channel]);
                destination_row[byte_offset + channel] = kernel.lut()[channel * 256 + sample];
            }
        }
    }
    Some(Mapped {
        output: Output { width: kernel.width(), height: kernel.height(), bytes },
        vector_blocks: u64::try_from(vector_width / LANES * height).unwrap_or(u64::MAX),
        scalar_tail: None,
    })
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
#[allow(unsafe_code)]
fn neon_rgb_affine_map(
    kernel: &Kernel<'_, 3>,
    coefficients: [(u8, u8); 3],
) -> Option<Mapped> {
    use core::arch::aarch64 as neon;

    const CHANNELS_RGB: usize = 3;
    const LANES: usize = 16;
    let row_stride = (kernel.width() as usize).checked_mul(CHANNELS_RGB)?;
    let expected_len = row_stride.checked_mul(kernel.height() as usize)?;
    if row_stride == 0 || kernel.source().len() != expected_len {
        return None;
    }
    // SAFETY: duplicate intrinsics only construct vector constants.
    let multipliers = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].0),
            neon::vdupq_n_u8(coefficients[1].0),
            neon::vdupq_n_u8(coefficients[2].0),
        ]
    };
    // SAFETY: duplicate intrinsics only construct vector constants.
    let offsets = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].1),
            neon::vdupq_n_u8(coefficients[1].1),
            neon::vdupq_n_u8(coefficients[2].1),
        ]
    };
    let width = kernel.width() as usize;
    let height = kernel.height() as usize;
    let vector_width = width / LANES * LANES;
    let mut bytes = vec![0u8; expected_len];
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &kernel.source()[row_start..row_start + row_stride];
        let destination_row = &mut bytes[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS_RGB;
            // SAFETY: vector_width is rounded down to complete groups of
            // sixteen RGB pixels, so both 48-byte accesses stay in this row.
            // The source and destination slices are disjoint and NEON is
            // enabled by this function's compile-time target gate.
            unsafe {
                let rgb = neon::vld3q_u8(source_row.as_ptr().add(byte_offset));
                let mapped = neon::uint8x16x3_t(
                    neon::vaddq_u8(neon::vmulq_u8(rgb.0, multipliers[0]), offsets[0]),
                    neon::vaddq_u8(neon::vmulq_u8(rgb.1, multipliers[1]), offsets[1]),
                    neon::vaddq_u8(neon::vmulq_u8(rgb.2, multipliers[2]), offsets[2]),
                );
                neon::vst3q_u8(destination_row.as_mut_ptr().add(byte_offset), mapped);
            }
        }
        for pixel in vector_width..width {
            let byte_offset = pixel * CHANNELS_RGB;
            for channel in 0..CHANNELS_RGB {
                let (multiplier, offset) = coefficients[channel];
                destination_row[byte_offset + channel] = source_row[byte_offset + channel]
                    .wrapping_mul(multiplier)
                    .wrapping_add(offset);
            }
        }
    }
    Some(Mapped {
        output: Output { width: kernel.width(), height: kernel.height(), bytes },
        vector_blocks: u64::try_from(vector_width / LANES * height).unwrap_or(u64::MAX),
        scalar_tail: None,
    })
}
