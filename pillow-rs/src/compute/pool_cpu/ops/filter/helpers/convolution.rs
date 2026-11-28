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

/// Evaluate Pillow's three-tap row expression with the arithmetic available
/// in its build: x86 keeps the C multiply/add sequence, while other targets
/// use Pillow's observed fused order.
///
/// The difference is observable for I-mode Detail: the x86 Pillow build
/// returns 89 for the center sample while the pinned arm64 build returns 90.
/// Keeping this compile-time choice explicit avoids target-feature checks in
/// pixel loops and preserves each architecture's Pillow output.
// VERIFY: source=reviewed@2026-11-28; parity=pass@filter-architecture-closure-20261128; benchmark=pending
#[inline]
pub(crate) fn pillow_kernel_row_3(pixels: [f32; 3], kernel: &[f32]) -> f32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return pixels[0] * kernel[0] + pixels[1] * kernel[1] + pixels[2] * kernel[2];
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        let sum = pixels[1] * kernel[1];
        let sum = pixels[0].mul_add(kernel[0], sum);
        pixels[2].mul_add(kernel[2], sum)
    }
}

/// Five-tap counterpart of [`pillow_kernel_row_3`].
///
/// The x86 Pillow kernel uses its C expression's left-associated multiply and
/// add sequence. Other targets retain the fused order observed in Pillow's
/// arm64 build.
// VERIFY: source=reviewed@2026-11-28; parity=pass@filter-architecture-closure-20261128; benchmark=pending
#[inline]
pub(crate) fn pillow_kernel_row_5(pixels: [f32; 5], kernel: &[f32]) -> f32 {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let sum = pixels[0] * kernel[0] + pixels[1] * kernel[1];
        let sum = sum + pixels[2] * kernel[2];
        let sum = sum + pixels[3] * kernel[3];
        sum + pixels[4] * kernel[4]
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        let sum = pixels[1] * kernel[1];
        let sum = pixels[0].mul_add(kernel[0], sum);
        let sum = pixels[2].mul_add(kernel[2], sum);
        let sum = pixels[3].mul_add(kernel[3], sum);
        pixels[4].mul_add(kernel[4], sum)
    }
}
