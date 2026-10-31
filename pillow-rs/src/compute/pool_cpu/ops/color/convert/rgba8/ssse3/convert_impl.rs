//! SSSE3 RGBA-to-RGB conversion for the exact RGBA8 source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage, RgbaImage};

/// Executes the SSSE3 RGBA-to-RGB kernel, with the archived wide-layout fallback.
///
/// # Safety
///
/// The caller must ensure that the host supports SSSE3.
#[cfg(target_arch = "x86_64")]
pub(crate) unsafe fn execute(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert dimensions overflowed".into()))?;
    let source_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert source size overflowed".into()))?;
    let output_bytes = pixels
        .checked_mul(3)
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| PilError::DimensionError("RGB Convert output size overflowed".into()))?;
    if source.as_raw().len() != source_bytes {
        return Err(PilError::InternalError(
            "RGBA Convert source storage did not match its dimensions".into(),
        ));
    }

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::new();
    if output.try_reserve_exact(output_bytes).is_err() {
        // The archived adapter falls through to its portable layout kernel
        // when this specialization cannot reserve its output buffer.
        return super::super::helpers::vector::execute(source, Mode::ImageRgb8);
    }
    // SAFETY: the mode strategy selected SSSE3 only after the execution
    // context confirmed the host feature. Exact source length and output
    // capacity are checked above; the kernel writes complete 16-pixel blocks.
    unsafe { rgba_to_rgb_ssse3(source.as_raw(), vector_pixels, &mut output) };
    for pixel in vector_pixels..pixels {
        let offset = pixel * 4;
        output.extend_from_slice(&source.as_raw()[offset..offset + 3]);
    }

    if pixels == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks((vector_pixels / 16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_pixels) as u64);
        crate::compute::record_pipeline_operation_path("ssse3-rgba-to-rgb");
    }
    RgbImage::from_raw(source.width(), source.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("RGB Convert output shape mismatch".into()))
}

#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "ssse3",
    not(target_feature = "avx2")
))]
pub(crate) fn execute_luma(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl::prepare(
        source,
    )?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgba8::ssse3::grayscale_impl::execute(
        kernel,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
#[allow(unsafe_code)]
unsafe fn rgba_to_rgb_ssse3(source: &[u8], vector_pixels: usize, output: &mut Vec<u8>) {
    use core::arch::x86_64::{
        _mm_and_si128, _mm_loadu_si128, _mm_or_si128, _mm_setr_epi8, _mm_shuffle_epi8,
        _mm_slli_si128, _mm_srli_si128, _mm_storeu_si128,
    };

    // SAFETY: the caller validated four source bytes per pixel, reserved the
    // full output, and selected this target-feature function through SSSE3.
    unsafe {
        let rgb_lanes = _mm_setr_epi8(0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, -1, -1, -1, -1);
        let low_twelve =
            _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 0, 0, 0);
        let output_pointer = output.as_mut_ptr();
        for pixel in (0..vector_pixels).step_by(16) {
            let source_start = pixel * 4;
            let input0 = _mm_loadu_si128(source.as_ptr().add(source_start).cast());
            let input1 = _mm_loadu_si128(source.as_ptr().add(source_start + 16).cast());
            let input2 = _mm_loadu_si128(source.as_ptr().add(source_start + 32).cast());
            let input3 = _mm_loadu_si128(source.as_ptr().add(source_start + 48).cast());
            let rgb0 = _mm_shuffle_epi8(input0, rgb_lanes);
            let rgb1 = _mm_shuffle_epi8(input1, rgb_lanes);
            let rgb2 = _mm_shuffle_epi8(input2, rgb_lanes);
            let rgb3 = _mm_shuffle_epi8(input3, rgb_lanes);
            let output_start = output_pointer.add(pixel * 3);
            let packed0 = _mm_or_si128(_mm_and_si128(rgb0, low_twelve), _mm_slli_si128::<12>(rgb1));
            let packed1 = _mm_or_si128(_mm_srli_si128::<4>(rgb1), _mm_slli_si128::<8>(rgb2));
            let packed2 = _mm_or_si128(_mm_srli_si128::<8>(rgb2), _mm_slli_si128::<4>(rgb3));
            _mm_storeu_si128(output_start.cast(), packed0);
            _mm_storeu_si128(output_start.add(16).cast(), packed1);
            _mm_storeu_si128(output_start.add(32).cast(), packed2);
        }
        output.set_len(vector_pixels * 3);
    }
}
