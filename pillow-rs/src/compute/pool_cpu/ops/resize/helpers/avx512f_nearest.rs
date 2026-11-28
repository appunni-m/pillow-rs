//! AVX-512F nearest gather for exact four-byte sample layouts.

#![allow(unsafe_code)] // Mode dispatch validates AVX-512F before entering the intrinsic leaf.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::resize::helpers::strategy::require_architecture;
use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{Mode, RgbaImage};

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 16;
const SAMPLE_BYTES: usize = 4;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    selection: &StrategySelection,
    expected_mode: Mode,
    input: &RgbaImage,
    output_size: (u32, u32),
) -> Result<RgbaImage, PilError> {
    let architecture = require_architecture(context, selection)?;
    if architecture != Architecture::Avx512F
        || selection.implementation.operation != OperationId::Resize
        || selection.implementation.mode != expected_mode
        || selection.guard != StrategyGuard::ResizeAvx512Nearest
    {
        return Err(PilError::InternalError(
            "AVX-512F nearest Resize received a mismatched implementation selection".into(),
        ));
    }
    let source_dims = CheckedDims::new(input.width(), input.height(), SAMPLE_BYTES as u8)?;
    let output_dims = CheckedDims::new(output_size.0, output_size.1, SAMPLE_BYTES as u8)?;
    if input.as_raw().len() != source_dims.total_bytes() || input.width() > i32::MAX as u32 {
        return Err(invalid_layout());
    }

    let source_width = usize::try_from(input.width()).map_err(|_| invalid_layout())?;
    let output_width = usize::try_from(output_size.0).map_err(|_| invalid_layout())?;
    let x_indices = super::native_nearest::nearest_indices(input.width(), output_size.0)
        .ok_or_else(invalid_layout)?;
    let y_indices = super::native_nearest::nearest_indices(input.height(), output_size.1)
        .ok_or_else(invalid_layout)?;
    let mut output = output_dims.alloc_buffer();
    let vector_width = output_width / LANES * LANES;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .and_then(|offset| offset.checked_mul(SAMPLE_BYTES))
            .ok_or_else(invalid_layout)?;
        let output_row = output_y
            .checked_mul(output_width)
            .and_then(|offset| offset.checked_mul(SAMPLE_BYTES))
            .ok_or_else(invalid_layout)?;
        for output_x in (0..vector_width).step_by(LANES) {
            let mut indices = [0i32; LANES];
            for (lane, index) in indices.iter_mut().enumerate() {
                *index = i32::try_from(x_indices[output_x + lane]).map_err(|_| invalid_layout())?;
            }
            // SAFETY: the source row is validated by CheckedDims, every gather index comes
            // from nearest_indices and is below source_width, the destination has a complete
            // 64-byte block, and this helper validated the injected AVX-512F context above.
            unsafe {
                gather_store_block(
                    input.as_raw().as_ptr().add(source_row),
                    indices.as_ptr(),
                    output
                        .as_mut_ptr()
                        .add(output_row + output_x * SAMPLE_BYTES),
                );
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
        for output_x in vector_width..output_width {
            let source_offset = source_row + x_indices[output_x] * SAMPLE_BYTES;
            let output_offset = output_row + output_x * SAMPLE_BYTES;
            output[output_offset..output_offset + SAMPLE_BYTES]
                .copy_from_slice(&input.as_raw()[source_offset..source_offset + SAMPLE_BYTES]);
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    RgbaImage::from_raw(output_size.0, output_size.1, output)
        .ok_or_else(|| PilError::ValueError("AVX-512F nearest Resize output shape mismatch".into()))
}

fn invalid_layout() -> PilError {
    PilError::InternalError("AVX-512F nearest Resize received invalid four-byte storage".into())
}

/// # Safety
/// The caller must provide AVX-512F, sixteen valid source indices, a readable
/// source row containing each selected sample, and a writable 64-byte output.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn gather_store_block(source_row: *const u8, indices: *const i32, output: *mut u8) {
    // SAFETY: caller guarantees both pointer ranges; gather supports unaligned memory addresses.
    let lanes = unsafe { arch::_mm512_loadu_si512(indices.cast()) };
    let samples = unsafe { arch::_mm512_i32gather_epi32::<4>(lanes, source_row.cast()) };
    unsafe { arch::_mm512_storeu_si512(output.cast(), samples) };
}
