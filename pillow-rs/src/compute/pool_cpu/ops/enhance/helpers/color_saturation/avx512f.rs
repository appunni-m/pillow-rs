//! AVX-512F lane expansion for exact native-byte ColorSaturation strategies.

#![allow(unsafe_code)] // Callers select this function through the injected AVX-512F strategy.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute<P, const CHANNELS: u8, const BASE: u8>(
    source: &ImageBuffer<P, Vec<u8>>,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), CHANNELS)?;
    let source_bytes = source.as_raw();
    let factor_f32 = factor as f32;
    if source_bytes.len() != dimensions.total_bytes()
        || !factor.is_finite()
        || !factor_f32.is_finite()
        || source_bytes.len() < 16
    {
        return Err(PilError::InternalError(
            "ColorSaturation AVX-512F received an ineligible native layout".into(),
        ));
    }
    if CHANNELS == 4 && BASE == super::BASE_RGB && factor == 0.0 {
        return super::rgba_color_degenerate(source);
    }

    let vector_bytes = source_bytes.len() / 16 * 16;
    let mut output = vec![0; source_bytes.len()];
    for offset in (0..vector_bytes).step_by(16) {
        let bases: [i32; 16] = std::array::from_fn(|lane| {
            let index = offset + lane;
            let channels = usize::from(CHANNELS);
            let pixel_start = index / channels * channels;
            i32::from(
                super::pixel_base::<BASE>(&source_bytes[pixel_start..pixel_start + channels])
                    [index % channels],
            )
        });
        // SAFETY: the guard requires AVX-512F and a complete sixteen-byte input/output block.
        unsafe {
            apply_block(
                source_bytes.as_ptr().add(offset),
                bases.as_ptr(),
                output.as_mut_ptr().add(offset),
                factor_f32,
            );
        }
    }
    for (tail_index, destination) in output[vector_bytes..].iter_mut().enumerate() {
        let index = vector_bytes + tail_index;
        let channels = usize::from(CHANNELS);
        let pixel_start = index / channels * channels;
        let base = f32::from(
            super::pixel_base::<BASE>(&source_bytes[pixel_start..pixel_start + channels])
                [index % channels],
        );
        *destination = factor_f32
            .mul_add(f32::from(source_bytes[index]) - base, base)
            .clamp(0.0, 255.0) as u8;
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_bytes / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(
        (source_bytes.len() - vector_bytes) as u64,
    );
    crate::compute::record_pipeline_operation_path("avx512f");
    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("ColorSaturation output dimensions were invalid".into())
    })
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn apply_block(input: *const u8, bases: *const i32, output: *mut u8, factor: f32) {
    // SAFETY: the selected strategy validates all three sixteen-lane memory ranges.
    unsafe {
        let input_bytes = arch::_mm_loadu_si128(input.cast());
        let input = arch::_mm512_cvtepu8_epi32(input_bytes);
        let base_i32 = arch::_mm512_loadu_si512(bases.cast());
        let base = arch::_mm512_cvtepi32_ps(base_i32);
        let samples = arch::_mm512_cvtepi32_ps(input);
        let factor = arch::_mm512_set1_ps(factor);
        let value = arch::_mm512_fmadd_ps(factor, arch::_mm512_sub_ps(samples, base), base);
        let value = arch::_mm512_min_ps(
            arch::_mm512_set1_ps(255.0),
            arch::_mm512_max_ps(arch::_mm512_setzero_ps(), value),
        );
        let value = arch::_mm512_cvttps_epi32(value);
        let mut lanes = [0i32; 16];
        arch::_mm512_storeu_si512(lanes.as_mut_ptr().cast(), value);
        for (lane, sample) in lanes.into_iter().enumerate() {
            output.add(lane).write(sample as u8);
        }
    }
}
