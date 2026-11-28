//! AVX-512F gather for axis-swapping four-byte pixels.

#![allow(unsafe_code)] // Prepared exact-mode strategies guard this target-feature leaf.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::TransposeMethod;

#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 16;
const MAX_PIXELS: u64 = i32::MAX as u64;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn transpose_native32(
    raw: &[u8],
    width: u32,
    height: u32,
    method: TransposeMethod,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    let source_dims = CheckedDims::new(width, height, 4)?;
    let pixels = u64::from(width) * u64::from(height);
    if width < LANES as u32 || height < LANES as u32 || pixels > MAX_PIXELS {
        return Err(PilError::InternalError(
            "AVX-512F transpose received an ineligible image shape".into(),
        ));
    }
    let source = raw.get(..source_dims.total_bytes()).ok_or_else(|| {
        PilError::InternalError("AVX-512F transpose received short pixel storage".into())
    })?;
    let (reverse_source_x, reverse_source_y) = match method {
        TransposeMethod::Rotate90 => (true, false),
        TransposeMethod::Rotate270 => (false, true),
        TransposeMethod::Transpose => (false, false),
        TransposeMethod::Transverse => (true, true),
        _ => {
            return Err(PilError::InternalError(
                "AVX-512F transpose received a non-axis-swapping method".into(),
            ));
        }
    };
    let (output_width, output_height) = (height, width);
    let output_dims = CheckedDims::new(output_width, output_height, 4)?;
    let mut output = output_dims.alloc_buffer();
    // SAFETY: the prepared exact-mode guard validates axis-swapping methods and the i32 index bound.
    unsafe {
        transpose_rows(
            source,
            &mut output,
            width as usize,
            height as usize,
            reverse_source_x,
            reverse_source_y,
        )
    };
    let vector_blocks = u64::from(output_width).div_ceil(LANES as u64) * u64::from(output_height);
    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok((output_width, output_height, output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn transpose_rows(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    reverse_source_x: bool,
    reverse_source_y: bool,
) {
    let output_width = height;
    let output_height = width;
    for output_y in 0..output_height {
        let source_x = if reverse_source_x {
            width - 1 - output_y
        } else {
            output_y
        };
        let mut output_x = 0usize;
        while output_x < output_width {
            let active = (output_width - output_x).min(LANES);
            let mut indices = [0i32; LANES];
            for (lane, index) in indices.iter_mut().enumerate() {
                let destination_x = (output_x + lane).min(output_width - 1);
                let source_y = if reverse_source_y {
                    height - 1 - destination_x
                } else {
                    destination_x
                };
                *index = (source_y * width + source_x) as i32;
            }
            let output_index = output_y * output_width + output_x;
            let mask = ((1u32 << active) - 1) as arch::__mmask16;
            // SAFETY: indices reference valid source dwords; the mask stores active output lanes.
            unsafe {
                let index_vector = arch::_mm512_loadu_si512(indices.as_ptr().cast());
                let values =
                    arch::_mm512_i32gather_epi32::<4>(index_vector, source.as_ptr().cast());
                arch::_mm512_mask_storeu_epi32(
                    output.as_mut_ptr().add(output_index * 4).cast(),
                    mask,
                    values,
                );
            }
            output_x += active;
        }
    }
}
