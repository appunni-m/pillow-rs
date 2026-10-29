//! Shared Pillow-compatible convolution arithmetic.

/// PIL's clip8: truncating cast to u8, clamping at 0 and 255.
/// Matches PIL ImagingFilter's clip8(): `return ss <= 0.0 ? 0 : ss >= 255.0 ? 255 : (UINT8)ss`
pub(super) fn clip8_filter(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

/// Evaluate Pillow's three-tap row expression with the contraction order used
/// by the pinned arm64 Pillow 12.2.0 oracle.
///
/// `src/libImaging/Filter.c` spells `KERNEL1x3` as a left-associated sum. The
/// oracle binary starts with the middle product, then emits fused
/// multiply-adds for the left and right products. Keeping that order explicit
/// avoids one-byte differences when a result lies just below an integer.
#[inline]
pub(super) fn pillow_kernel_row_3(pixels: [f32; 3], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    pixels[2].mul_add(kernel[2], sum)
}

/// Five-tap counterpart of [`pillow_kernel_row_3`].
///
/// The pinned oracle starts with tap 1, fuses tap 0, and then fuses taps 2
/// through 4 in source order.
#[inline]
pub(super) fn pillow_kernel_row_5(pixels: [f32; 5], kernel: &[f32]) -> f32 {
    let sum = pixels[1] * kernel[1];
    let sum = pixels[0].mul_add(kernel[0], sum);
    let sum = pixels[2].mul_add(kernel[2], sum);
    let sum = pixels[3].mul_add(kernel[3], sum);
    pixels[4].mul_add(kernel[4], sum)
}
