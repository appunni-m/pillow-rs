//! Sixteen-lane AVX-512F GetExtrema for native signed ImageI32 samples.

#![allow(unsafe_code)] // The exact-mode strategy selects AVX-512F before this leaf.

use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::image::FormattedExtrema;
use crate::raster::{Mode, RgbaImage};

const SAMPLE_BYTES: usize = 4;
const VECTOR_BYTES: usize = 64;
const VECTOR_SAMPLES: usize = VECTOR_BYTES / SAMPLE_BYTES;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &RgbaImage,
    selection: &StrategySelection,
) -> Result<FormattedExtrema, PilError> {
    validate(image, selection)?;
    let raw = image.as_raw();
    let vector_bytes = raw.len() / VECTOR_BYTES * VECTOR_BYTES;
    // SAFETY: validate confirms the selected AVX-512F strategy and complete storage layout.
    let (mut minimum, mut maximum) = unsafe { reduce_blocks(&raw[..vector_bytes]) };
    let mut scalar_tail = 0usize;
    for sample in raw[vector_bytes..].chunks_exact(SAMPLE_BYTES) {
        let value = i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        scalar_tail += 1;
    }
    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks((vector_bytes / VECTOR_BYTES) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    Ok(FormattedExtrema::Integer((minimum, maximum)))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn validate(image: &RgbaImage, selection: &StrategySelection) -> Result<(), PilError> {
    let dimensions = CheckedDims::new(image.width(), image.height(), SAMPLE_BYTES as u8)?;
    let implementation = selection.implementation;
    if selection.requested_backend != Backend::Cpu
        || selection.requested_architecture != Architecture::Avx512F
        || implementation.backend != Backend::Cpu
        || implementation.operation != OperationId::GetExtrema
        || implementation.mode != Mode::ImageI32
        || implementation.architecture != Architecture::Avx512F
        || selection.guard != StrategyGuard::GetExtremaI32Avx512F
        || image.as_raw().len() != dimensions.total_bytes()
        || image.as_raw().len() < VECTOR_BYTES
    {
        return Err(PilError::InternalError(
            "ImageI32 AVX-512F GetExtrema received an ineligible strategy request".into(),
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx512f")]
unsafe fn reduce_blocks(raw: &[u8]) -> (i32, i32) {
    #[cfg(target_arch = "x86")]
    use core::arch::x86 as arch;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64 as arch;

    // SAFETY: callers provide complete 64-byte blocks and dispatch only with AVX-512F enabled.
    unsafe {
        let mut minima = arch::_mm512_set1_epi32(i32::MAX);
        let mut maxima = arch::_mm512_set1_epi32(i32::MIN);
        for offset in (0..raw.len()).step_by(VECTOR_BYTES) {
            let values = arch::_mm512_loadu_si512(raw.as_ptr().add(offset).cast());
            minima = arch::_mm512_min_epi32(minima, values);
            maxima = arch::_mm512_max_epi32(maxima, values);
        }
        let mut min_lanes = [0_i32; VECTOR_SAMPLES];
        let mut max_lanes = [0_i32; VECTOR_SAMPLES];
        arch::_mm512_storeu_si512(min_lanes.as_mut_ptr().cast(), minima);
        arch::_mm512_storeu_si512(max_lanes.as_mut_ptr().cast(), maxima);
        (
            min_lanes.into_iter().min().unwrap_or(i32::MAX),
            max_lanes.into_iter().max().unwrap_or(i32::MIN),
        )
    }
}
