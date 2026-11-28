//! AVX-512F planar-to-four-byte interleave for exact RGBA and CMYK modes.

#![allow(unsafe_code)] // Exact mode strategies admit the AVX-512F leaf.

use super::super::native_bytes;
use super::{MergeLayout, collect_bands, preserve_merged_padding};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

const PIXELS_PER_BLOCK: usize = 16;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn interleave_four(
    first: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    if !matches!(mode, Mode::ImageRgba8 | Mode::ImageCmyk8) {
        return Err(PilError::InternalError(
            "AVX-512F Merge received an unsupported output mode".into(),
        ));
    }
    let (width, height) = (first.width(), first.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, 4)?;
    let pixels = dimensions.total_bytes() / 4;
    let bands = collect_bands(first, remaining, pixels)?;
    let [red, green, blue, alpha] = bands.as_slice() else {
        return Err(PilError::InternalError(
            "AVX-512F Merge requires exactly four L bands".into(),
        ));
    };

    let vector_pixels = pixels / PIXELS_PER_BLOCK * PIXELS_PER_BLOCK;
    let mut output = dimensions.alloc_buffer();
    for pixel in (0..vector_pixels).step_by(PIXELS_PER_BLOCK) {
        let byte_offset = pixel * 4;
        // SAFETY: checked dimensions and collected bands cover every complete
        // 16-pixel input block and its corresponding 64-byte output block.
        unsafe {
            interleave_four_block(
                red.as_ptr().add(pixel),
                green.as_ptr().add(pixel),
                blue.as_ptr().add(pixel),
                alpha.as_ptr().add(pixel),
                output.as_mut_ptr().add(byte_offset),
            );
        }
    }
    for pixel in vector_pixels..pixels {
        let output_start = pixel * 4;
        output[output_start] = red[pixel];
        output[output_start + 1] = green[pixel];
        output[output_start + 2] = blue[pixel];
        output[output_start + 3] = alpha[pixel];
    }

    let output_bytes = dimensions.total_bytes();
    crate::compute::record_pipeline_allocation(output_bytes);
    crate::compute::record_pipeline_operation_vector_blocks(
        (vector_pixels / PIXELS_PER_BLOCK) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(((pixels - vector_pixels) * 4) as u64);
    crate::compute::record_pipeline_operation_path("avx512f");
    let mut output = native_bytes::construct(width, height, mode, output)?;
    preserve_merged_padding(&mut output, mode, MergeLayout::Four, pixels)?;
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn interleave_two(
    first: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    if mode != Mode::ImageLumaA8 {
        return Err(PilError::InternalError(
            "AVX-512F Merge received an unsupported two-channel output mode".into(),
        ));
    }
    let (width, height) = (first.width(), first.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, 2)?;
    let pixels = dimensions.total_bytes() / 2;
    let bands = collect_bands(first, remaining, pixels)?;
    let [luma, alpha] = bands.as_slice() else {
        return Err(PilError::InternalError(
            "AVX-512F LA Merge requires exactly two L bands".into(),
        ));
    };

    let vector_pixels = pixels / PIXELS_PER_BLOCK * PIXELS_PER_BLOCK;
    let mut output = dimensions.alloc_buffer();
    for pixel in (0..vector_pixels).step_by(PIXELS_PER_BLOCK) {
        // SAFETY: collected input bands provide 16 readable bytes per plane,
        // and the output provides 32 writable bytes for this complete block.
        unsafe {
            interleave_two_block(
                luma.as_ptr().add(pixel),
                alpha.as_ptr().add(pixel),
                output.as_mut_ptr().add(pixel * 2),
            );
        }
    }
    for pixel in vector_pixels..pixels {
        output[pixel * 2] = luma[pixel];
        output[pixel * 2 + 1] = alpha[pixel];
    }

    crate::compute::record_pipeline_allocation(dimensions.total_bytes());
    crate::compute::record_pipeline_operation_vector_blocks(
        (vector_pixels / PIXELS_PER_BLOCK) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(((pixels - vector_pixels) * 2) as u64);
    crate::compute::record_pipeline_operation_path("avx512f");
    let mut output = native_bytes::construct(width, height, mode, output)?;
    preserve_merged_padding(&mut output, mode, MergeLayout::Two, pixels)?;
    Ok(output)
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn interleave_four_block(
    red: *const u8,
    green: *const u8,
    blue: *const u8,
    alpha: *const u8,
    output: *mut u8,
) {
    let red = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(red.cast())) };
    let green = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(green.cast())) };
    let blue = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(blue.cast())) };
    let alpha = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(alpha.cast())) };
    let green = { arch::_mm512_slli_epi32(green, 8) };
    let blue = { arch::_mm512_slli_epi32(blue, 16) };
    let alpha = { arch::_mm512_slli_epi32(alpha, 24) };
    let red_green = { arch::_mm512_or_si512(red, green) };
    let blue_alpha = { arch::_mm512_or_si512(blue, alpha) };
    let pixels = { arch::_mm512_or_si512(red_green, blue_alpha) };
    unsafe { arch::_mm512_storeu_si512(output.cast(), pixels) };
}

/// # Safety
/// Both sources must provide 16 readable bytes and output must provide 32
/// writable bytes. The caller must establish AVX-512F support.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn interleave_two_block(luma: *const u8, alpha: *const u8, output: *mut u8) {
    let luma = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(luma.cast())) };
    let alpha = unsafe { arch::_mm512_cvtepu8_epi32(arch::_mm_loadu_si128(alpha.cast())) };
    let alpha = { arch::_mm512_slli_epi32(alpha, 8) };
    let pixels = { arch::_mm512_or_si512(luma, alpha) };
    let packed = { arch::_mm512_cvtepi32_epi16(pixels) };
    unsafe { arch::_mm256_storeu_si256(output.cast(), packed) };
}
