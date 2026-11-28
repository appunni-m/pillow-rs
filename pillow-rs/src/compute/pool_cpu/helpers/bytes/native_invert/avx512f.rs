//! AVX-512F byte inversion leaf shared by exact-mode CPU operation adapters.

#![allow(unsafe_code)] // The prepared ImageOps or Chops strategy establishes AVX-512F.

use super::NativeInvertOutput;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;

#[cfg(target_arch = "x86")]
use std::arch::x86 as x86_arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as x86_arch;

const VECTOR_BYTES: usize = 64;

// TODO(pillow-rs#3ptb): confirm exact-mode ImageOps and Chops outputs, 63/64-byte
// row routing, fallback receipts, and per-mode latency on the frozen CPU matrix.
/// Invert the declared native byte prefix using AVX-512F dword XOR blocks.
///
/// The prepared operation strategy establishes AVX-512F before reaching this
/// leaf. Partial rows retain the same per-channel behavior as the shared
/// native-byte implementation.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn invert(
    source: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    invert_alpha: bool,
) -> Result<NativeInvertOutput, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, channels as u8)?;
    let expected_bytes = dimensions.total_bytes();
    if source.len() < expected_bytes {
        return Err(PilError::InternalError(
            "invert native byte prefix is short".into(),
        ));
    }
    if source.is_empty() {
        return Ok(NativeInvertOutput::Empty);
    }

    let mask = dword_mask(channels, invert_alpha)?;
    let row_bytes = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("invert native row size overflow".into()))?;
    let mut output = vec![0; source.len()];
    if row_bytes == 0 || height == 0 {
        output.copy_from_slice(source);
        return Ok(output_kind(output, expected_bytes, source.len()));
    }

    let (blocks, scalar_tail) = invert_rows(
        &source[..expected_bytes],
        &mut output[..expected_bytes],
        row_bytes,
        height as usize,
        channels,
        invert_alpha,
        mask,
    );
    output[expected_bytes..].copy_from_slice(&source[expected_bytes..]);
    crate::compute::record_pipeline_operation_vector_blocks(blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    Ok(output_kind(output, expected_bytes, source.len()))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn output_kind(output: Vec<u8>, expected_bytes: usize, source_bytes: usize) -> NativeInvertOutput {
    if source_bytes == expected_bytes {
        NativeInvertOutput::Exact(output)
    } else {
        NativeInvertOutput::Prefix(output)
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn dword_mask(channels: usize, invert_alpha: bool) -> Result<i32, PilError> {
    match (channels, invert_alpha) {
        (1 | 3, _) | (2 | 4, true) => Ok(-1),
        (2, false) => Ok(0x00ff_00ff),
        (4, false) => Ok(0x00ff_ffff),
        _ => Err(PilError::InternalError(
            "invert native channel count is invalid".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn invert_rows(
    source: &[u8],
    output: &mut [u8],
    row_bytes: usize,
    height: usize,
    channels: usize,
    invert_alpha: bool,
    mask: i32,
) -> (usize, usize) {
    let mut vector_blocks = 0usize;
    let mut scalar_tail = 0usize;
    for (source_row, output_row) in source
        .chunks_exact(row_bytes)
        .zip(output.chunks_exact_mut(row_bytes))
        .take(height)
    {
        let full_bytes = source_row.len() / VECTOR_BYTES * VECTOR_BYTES;
        if full_bytes != 0 {
            // SAFETY: the prepared mode strategy selects this leaf only when
            // AVX-512F is available and both slices contain complete 64-byte blocks.
            unsafe {
                invert_full_blocks(
                    &source_row[..full_bytes],
                    &mut output_row[..full_bytes],
                    mask,
                );
            }
            vector_blocks += full_bytes / VECTOR_BYTES;
        }
        for (index, (source, destination)) in source_row[full_bytes..]
            .iter()
            .zip(&mut output_row[full_bytes..])
            .enumerate()
        {
            if invert_channel(index, channels, invert_alpha) {
                *destination = !*source;
            } else {
                *destination = *source;
            }
            scalar_tail += 1;
        }
    }
    (vector_blocks, scalar_tail)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn invert_channel(index: usize, channels: usize, invert_alpha: bool) -> bool {
    match channels {
        1 | 3 => true,
        2 => invert_alpha || index % 2 == 0,
        4 => invert_alpha || index % 4 != 3,
        _ => false,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
#[allow(unsafe_code)] // The prepared ImageOps or Chops strategy establishes AVX-512F.
unsafe fn invert_full_blocks(source: &[u8], output: &mut [u8], mask: i32) {
    use x86_arch::{_mm512_loadu_si512, _mm512_set1_epi32, _mm512_storeu_si512, _mm512_xor_si512};

    // SAFETY: the function's target feature is enabled; paired blocks are 64 bytes each.
    unsafe {
        let vector_mask = _mm512_set1_epi32(mask);
        for (source_block, output_block) in source
            .chunks_exact(VECTOR_BYTES)
            .zip(output.chunks_exact_mut(VECTOR_BYTES))
        {
            let source_pointer = source_block.as_ptr().cast::<x86_arch::__m512i>();
            let output_pointer = output_block.as_mut_ptr().cast::<x86_arch::__m512i>();
            let value = _mm512_loadu_si512(source_pointer);
            _mm512_storeu_si512(output_pointer, _mm512_xor_si512(value, vector_mask));
        }
    }
}
