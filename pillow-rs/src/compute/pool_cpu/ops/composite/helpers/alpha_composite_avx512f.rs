//! AVX-512F implementation of Pillow's fixed-point alpha-composite arithmetic.

#![allow(unsafe_code)] // Mode strategies admit AVX-512F before these leaves run.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

const RGBA_BLOCK_PIXELS: usize = 16;
const LA_BLOCK_PIXELS: usize = 32;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn apply_rgba(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PilError> {
    let dimensions = checked_layout(source, destination, width, height, 4, "RGBA")?;
    let pixels = dimensions.total_bytes() / 4;
    let vector_pixels = pixels / RGBA_BLOCK_PIXELS * RGBA_BLOCK_PIXELS;
    let mut output = destination.to_vec();
    for start in (0..vector_pixels).step_by(RGBA_BLOCK_PIXELS) {
        let byte_offset = start * 4;
        // SAFETY: the exact RGBA strategy selects AVX-512F and each call spans
        // one complete 16-pixel, 64-byte block in all three checked slices.
        unsafe {
            rgba_block(
                source.as_ptr().add(byte_offset),
                destination.as_ptr().add(byte_offset),
                output.as_mut_ptr().add(byte_offset),
            );
        }
    }
    for pixel in vector_pixels..pixels {
        let start = pixel * 4;
        super::alpha_composite::alpha_composite_pixel::<4>(
            &source[start..start + 4],
            &mut output[start..start + 4],
        );
    }
    record_execution(vector_pixels / RGBA_BLOCK_PIXELS, pixels - vector_pixels);
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn apply_la(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PilError> {
    let dimensions = checked_layout(source, destination, width, height, 2, "LA")?;
    let pixels = dimensions.total_bytes() / 2;
    let vector_pixels = pixels / LA_BLOCK_PIXELS * LA_BLOCK_PIXELS;
    let mut output = destination.to_vec();
    for start in (0..vector_pixels).step_by(LA_BLOCK_PIXELS) {
        let byte_offset = start * 2;
        // SAFETY: the exact LA strategy selects AVX-512F and each call spans
        // one complete 32-pixel, 64-byte block in all three checked slices.
        unsafe {
            la_block(
                source.as_ptr().add(byte_offset),
                destination.as_ptr().add(byte_offset),
                output.as_mut_ptr().add(byte_offset),
            );
        }
    }
    for pixel in vector_pixels..pixels {
        let start = pixel * 2;
        super::alpha_composite::alpha_composite_pixel::<2>(
            &source[start..start + 2],
            &mut output[start..start + 2],
        );
    }
    record_execution(vector_pixels / LA_BLOCK_PIXELS, pixels - vector_pixels);
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn checked_layout(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    mode_name: &str,
) -> Result<CheckedDims, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(format!(
            "{mode_name} AlphaComposite source storage did not match its dimensions"
        )));
    }
    Ok(dimensions)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn record_execution(vector_blocks: usize, scalar_tail: usize) {
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "scalar-control"
    } else {
        "avx512f"
    });
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn vector(value: i32) -> arch::__m512i {
    arch::_mm512_set1_epi32(value)
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn shift_for_div255(value: arch::__m512i) -> arch::__m512i {
    let shifted = { arch::_mm512_srli_epi32(value, 8) };
    let summed = { arch::_mm512_add_epi32(shifted, value) };
    arch::_mm512_srli_epi32(summed, 8)
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn coefficient(
    source_alpha: arch::__m512i,
    output_alpha_255: arch::__m512i,
) -> arch::__m512i {
    let zero = unsafe { vector(0) };
    let one = unsafe { vector(1) };
    let denominator_is_zero = { arch::_mm512_cmpeq_epi32_mask(output_alpha_255, zero) };
    let denominator = { arch::_mm512_mask_mov_epi32(output_alpha_255, denominator_is_zero, one) };
    let scale = unsafe { vector(255 * 255 * 128) };
    let numerator = { arch::_mm512_mullo_epi32(source_alpha, scale) };
    // F32 division supplies a close candidate; integer products correct it
    // both ways so the result is Pillow's exact unsigned floor quotient.
    let quotient = {
        arch::_mm512_cvttps_epi32(arch::_mm512_div_ps(
            arch::_mm512_cvtepi32_ps(numerator),
            arch::_mm512_cvtepi32_ps(denominator),
        ))
    };
    let product = { arch::_mm512_mullo_epi32(quotient, denominator) };
    let overestimated = { arch::_mm512_cmp_epi32_mask::<6>(product, numerator) };
    let quotient = { arch::_mm512_mask_sub_epi32(quotient, overestimated, quotient, one) };
    let next = { arch::_mm512_add_epi32(quotient, one) };
    let next_product = { arch::_mm512_mullo_epi32(next, denominator) };
    let underestimated = { arch::_mm512_cmp_epi32_mask::<2>(next_product, numerator) };
    arch::_mm512_mask_add_epi32(quotient, underestimated, quotient, one)
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn color_channel<const SHIFT: u32>(
    source: arch::__m512i,
    destination: arch::__m512i,
    coefficient_source: arch::__m512i,
    coefficient_destination: arch::__m512i,
) -> arch::__m512i {
    let byte = unsafe { vector(255) };
    let source = { arch::_mm512_and_si512(arch::_mm512_srli_epi32::<SHIFT>(source), byte) };
    let destination =
        { arch::_mm512_and_si512(arch::_mm512_srli_epi32::<SHIFT>(destination), byte) };
    let source_term = { arch::_mm512_mullo_epi32(source, coefficient_source) };
    let destination_term = { arch::_mm512_mullo_epi32(destination, coefficient_destination) };
    let blended = { arch::_mm512_add_epi32(source_term, destination_term) };
    let color_bias = unsafe { vector(0x80 << 7) };
    let biased = { arch::_mm512_add_epi32(blended, color_bias) };
    let rounded = unsafe { shift_for_div255(biased) };
    arch::_mm512_srli_epi32(rounded, 7)
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn output_alpha(output_alpha_255: arch::__m512i) -> arch::__m512i {
    let biased = unsafe { arch::_mm512_add_epi32(output_alpha_255, vector(0x80)) };
    unsafe { shift_for_div255(biased) }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn rgba_block(source: *const u8, destination: *const u8, output: *mut u8) {
    let source = unsafe { arch::_mm512_loadu_si512(source.cast()) };
    let destination = unsafe { arch::_mm512_loadu_si512(destination.cast()) };
    let byte = unsafe { vector(255) };
    let zero = unsafe { vector(0) };
    let source_alpha = { arch::_mm512_and_si512(arch::_mm512_srli_epi32(source, 24), byte) };
    let destination_alpha =
        { arch::_mm512_and_si512(arch::_mm512_srli_epi32(destination, 24), byte) };
    let inverse_source_alpha = { arch::_mm512_sub_epi32(byte, source_alpha) };
    let source_part = { arch::_mm512_mullo_epi32(source_alpha, byte) };
    let destination_part = { arch::_mm512_mullo_epi32(destination_alpha, inverse_source_alpha) };
    let output_alpha_255 = { arch::_mm512_add_epi32(source_part, destination_part) };
    let coefficient_source = unsafe { coefficient(source_alpha, output_alpha_255) };
    let coefficient_destination =
        unsafe { arch::_mm512_sub_epi32(vector(255 << 7), coefficient_source) };
    let mut result = unsafe {
        color_channel::<0>(
            source,
            destination,
            coefficient_source,
            coefficient_destination,
        )
    };
    let green = unsafe {
        color_channel::<8>(
            source,
            destination,
            coefficient_source,
            coefficient_destination,
        )
    };
    let blue = unsafe {
        color_channel::<16>(
            source,
            destination,
            coefficient_source,
            coefficient_destination,
        )
    };
    let alpha = unsafe { output_alpha(output_alpha_255) };
    result = arch::_mm512_or_si512(result, arch::_mm512_slli_epi32(green, 8));
    result = arch::_mm512_or_si512(result, arch::_mm512_slli_epi32(blue, 16));
    result = arch::_mm512_or_si512(result, arch::_mm512_slli_epi32(alpha, 24));
    let transparent = { arch::_mm512_cmpeq_epi32_mask(source_alpha, zero) };
    result = arch::_mm512_mask_mov_epi32(result, transparent, destination);
    unsafe { arch::_mm512_storeu_si512(output.cast(), result) };
}

#[inline]
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn composite_la_sample(
    source_luma: arch::__m512i,
    source_alpha: arch::__m512i,
    destination_luma: arch::__m512i,
    destination_alpha: arch::__m512i,
) -> arch::__m512i {
    let byte = unsafe { vector(255) };
    let source_part = { arch::_mm512_mullo_epi32(source_alpha, byte) };
    let inverse_source_alpha = { arch::_mm512_sub_epi32(byte, source_alpha) };
    let destination_part = { arch::_mm512_mullo_epi32(destination_alpha, inverse_source_alpha) };
    let output_alpha_255 = { arch::_mm512_add_epi32(source_part, destination_part) };
    let coefficient_source = unsafe { coefficient(source_alpha, output_alpha_255) };
    let coefficient_destination =
        unsafe { arch::_mm512_sub_epi32(vector(255 << 7), coefficient_source) };
    let source_term = { arch::_mm512_mullo_epi32(source_luma, coefficient_source) };
    let destination_term = { arch::_mm512_mullo_epi32(destination_luma, coefficient_destination) };
    let blended = { arch::_mm512_add_epi32(source_term, destination_term) };
    let color_bias = unsafe { vector(0x80 << 7) };
    let biased = { arch::_mm512_add_epi32(blended, color_bias) };
    let luma = unsafe { arch::_mm512_srli_epi32(shift_for_div255(biased), 7) };
    let alpha = unsafe { output_alpha(output_alpha_255) };
    let result = { arch::_mm512_or_si512(luma, arch::_mm512_slli_epi32(alpha, 8)) };
    let transparent = { arch::_mm512_cmpeq_epi32_mask(source_alpha, unsafe { vector(0) }) };
    let destination_pixel = {
        arch::_mm512_or_si512(
            destination_luma,
            arch::_mm512_slli_epi32(destination_alpha, 8),
        )
    };
    arch::_mm512_mask_mov_epi32(result, transparent, destination_pixel)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[target_feature(enable = "avx512f")]
unsafe fn la_block(source: *const u8, destination: *const u8, output: *mut u8) {
    let source = unsafe { arch::_mm512_loadu_si512(source.cast()) };
    let destination = unsafe { arch::_mm512_loadu_si512(destination.cast()) };
    let low_mask = unsafe { vector(0x0000_ffff) };
    let source_low = { arch::_mm512_and_si512(source, low_mask) };
    let destination_low = { arch::_mm512_and_si512(destination, low_mask) };
    let source_high = { arch::_mm512_srli_epi32(source, 16) };
    let destination_high = { arch::_mm512_srli_epi32(destination, 16) };
    let byte = unsafe { vector(255) };
    let source_low_luma = { arch::_mm512_and_si512(source_low, byte) };
    let source_low_alpha = { arch::_mm512_and_si512(arch::_mm512_srli_epi32(source_low, 8), byte) };
    let destination_low_luma = { arch::_mm512_and_si512(destination_low, byte) };
    let destination_low_alpha =
        { arch::_mm512_and_si512(arch::_mm512_srli_epi32(destination_low, 8), byte) };
    let source_high_luma = { arch::_mm512_and_si512(source_high, byte) };
    let source_high_alpha =
        { arch::_mm512_and_si512(arch::_mm512_srli_epi32(source_high, 8), byte) };
    let destination_high_luma = { arch::_mm512_and_si512(destination_high, byte) };
    let destination_high_alpha =
        { arch::_mm512_and_si512(arch::_mm512_srli_epi32(destination_high, 8), byte) };
    let low = unsafe {
        composite_la_sample(
            source_low_luma,
            source_low_alpha,
            destination_low_luma,
            destination_low_alpha,
        )
    };
    let high = unsafe {
        composite_la_sample(
            source_high_luma,
            source_high_alpha,
            destination_high_luma,
            destination_high_alpha,
        )
    };
    let result = { arch::_mm512_or_si512(low, arch::_mm512_slli_epi32(high, 16)) };
    unsafe { arch::_mm512_storeu_si512(output.cast(), result) };
}
