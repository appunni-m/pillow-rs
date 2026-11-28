//! AVX-512F bytewise native ImageOps kernels.

#![allow(unsafe_code)] // The exact-mode strategy selects AVX-512F before this leaf.

use crate::error::PilError;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn posterize(source: &[u8], bits: u8) -> Result<Vec<u8>, PilError> {
    if !(1..=8).contains(&bits) {
        return Err(PilError::ValueError(
            "posterize bits must be in range 1..=8".into(),
        ));
    }
    let mask = !((1u8 << u32::from(8 - bits)) - 1);
    let vector_bytes = source.len() / 64 * 64;
    let mut output = vec![0; source.len()];
    for offset in (0..vector_bytes).step_by(64) {
        // SAFETY: the Posterize guard admits complete 64-byte blocks and this
        // module is compiled only for x86 targets with an AVX-512F leaf.
        unsafe {
            posterize_block(
                source.as_ptr().add(offset),
                output.as_mut_ptr().add(offset),
                mask,
            );
        }
    }
    for (input, output) in source[vector_bytes..]
        .iter()
        .zip(&mut output[vector_bytes..])
    {
        *output = *input & mask;
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_bytes / 64) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((source.len() - vector_bytes) as u64);
    crate::compute::record_pipeline_operation_path("avx512f");
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn posterize_block(source: *const u8, output_ptr: *mut u8, mask: u8) {
    // SAFETY: callers provide one complete 64-byte source and destination block.
    unsafe {
        let input = arch::_mm512_loadu_si512(source.cast());
        let mask = arch::_mm512_set1_epi32(i32::from_ne_bytes([mask; 4]));
        let output = arch::_mm512_and_si512(input, mask);
        arch::_mm512_storeu_si512(output_ptr.cast(), output);
    }
}
