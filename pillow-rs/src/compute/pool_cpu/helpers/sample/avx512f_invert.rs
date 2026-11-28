//! AVX-512F bitwise inversion for native u16 and f32 Chops samples.

#![allow(unsafe_code)] // The exact-mode prepared strategy selects AVX-512F.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;

const VECTOR_BYTES: usize = 64;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn invert_u16<P: Pixel<Subpixel = u16>>(
    image: &ImageBuffer<P, Vec<u16>>,
) -> Result<ImageBuffer<P, Vec<u16>>, PilError> {
    let byte_len = checked_sample_layout(image, 2)?;
    let full_bytes = byte_len / VECTOR_BYTES * VECTOR_BYTES;
    let source_bytes = bytemuck::cast_slice(image.as_raw().as_slice());
    let mut output = image.as_raw().clone();
    let output_bytes = bytemuck::cast_slice_mut(output.as_mut_slice());
    // SAFETY: the prepared exact-mode strategy selects AVX-512F; both byte slices
    // are checked to contain the complete blocks passed to the intrinsic leaf.
    unsafe { invert_full_blocks(source_bytes, output_bytes, full_bytes) };
    let full_samples = full_bytes / 2;
    for (destination, &sample) in output[full_samples..]
        .iter_mut()
        .zip(&image.as_raw()[full_samples..])
    {
        *destination = !sample;
    }
    record_execution(full_bytes, output.len() - full_samples);
    ImageBuffer::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::InternalError("Chops u16 inversion shape mismatch".into()))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn invert_f32<P: Pixel<Subpixel = f32>>(
    image: &ImageBuffer<P, Vec<f32>>,
) -> Result<ImageBuffer<P, Vec<f32>>, PilError> {
    let byte_len = checked_sample_layout(image, 4)?;
    let full_bytes = byte_len / VECTOR_BYTES * VECTOR_BYTES;
    let source_bytes = bytemuck::cast_slice(image.as_raw().as_slice());
    let mut output = image.as_raw().clone();
    let output_bytes = bytemuck::cast_slice_mut(output.as_mut_slice());
    // SAFETY: the prepared exact-mode strategy selects AVX-512F; both byte slices
    // are checked to contain the complete blocks passed to the intrinsic leaf.
    unsafe { invert_full_blocks(source_bytes, output_bytes, full_bytes) };
    let full_samples = full_bytes / 4;
    for (destination, &sample) in output[full_samples..]
        .iter_mut()
        .zip(&image.as_raw()[full_samples..])
    {
        *destination = f32::from_bits(!sample.to_bits());
    }
    record_execution(full_bytes, output.len() - full_samples);
    ImageBuffer::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::InternalError("Chops f32 inversion shape mismatch".into()))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn checked_sample_layout<P, S>(
    image: &ImageBuffer<P, Vec<S>>,
    sample_bytes: u8,
) -> Result<usize, PilError>
where
    P: Pixel<Subpixel = S>,
{
    let bytes_per_pixel = usize::from(P::CHANNEL_COUNT)
        .checked_mul(usize::from(sample_bytes))
        .and_then(|bytes| u8::try_from(bytes).ok())
        .ok_or_else(|| PilError::InternalError("Chops sample width overflow".into()))?;
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), bytes_per_pixel)?;
    let byte_len = image
        .as_raw()
        .len()
        .checked_mul(usize::from(sample_bytes))
        .ok_or_else(|| PilError::InternalError("Chops sample byte length overflow".into()))?;
    if byte_len != dimensions.total_bytes() || byte_len < VECTOR_BYTES {
        return Err(PilError::InternalError(
            "Chops typed AVX-512F inversion received an ineligible layout".into(),
        ));
    }
    Ok(byte_len)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn record_execution(full_bytes: usize, scalar_samples: usize) {
    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks((full_bytes / VECTOR_BYTES) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_samples as u64);
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(target_arch = "x86")]
#[target_feature(enable = "avx512f")]
unsafe fn invert_full_blocks(source: &[u8], output: &mut [u8], full_bytes: usize) {
    use std::arch::x86::{
        _mm512_loadu_si512, _mm512_set1_epi32, _mm512_storeu_si512, _mm512_xor_si512,
    };

    // SAFETY: the caller supplies equal slices with complete 64-byte blocks.
    unsafe {
        let mask = _mm512_set1_epi32(-1);
        for offset in (0..full_bytes).step_by(VECTOR_BYTES) {
            let value = _mm512_loadu_si512(source.as_ptr().add(offset).cast());
            _mm512_storeu_si512(
                output.as_mut_ptr().add(offset).cast(),
                _mm512_xor_si512(value, mask),
            );
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn invert_full_blocks(source: &[u8], output: &mut [u8], full_bytes: usize) {
    use std::arch::x86_64::{
        _mm512_loadu_si512, _mm512_set1_epi32, _mm512_storeu_si512, _mm512_xor_si512,
    };

    // SAFETY: the caller supplies equal slices with complete 64-byte blocks.
    unsafe {
        let mask = _mm512_set1_epi32(-1);
        for offset in (0..full_bytes).step_by(VECTOR_BYTES) {
            let value = _mm512_loadu_si512(source.as_ptr().add(offset).cast());
            _mm512_storeu_si512(
                output.as_mut_ptr().add(offset).cast(),
                _mm512_xor_si512(value, mask),
            );
        }
    }
}
