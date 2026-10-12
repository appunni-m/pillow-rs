//! Native Y-channel extraction kernels.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_simd::ops::native_channel;
use crate::raster::DynamicImage;

/// Extract the Y sample from each exact native YCbCr byte triple. Use a
/// platform-specific vector deinterleave where available and keep incomplete
/// groups on the scalar tail.
pub(in crate::compute::pool_simd::ops::adapters) fn native_ycbcr_grayscale_bytes(
    img: &DynamicImage,
) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgb8(source) = img else {
        return None;
    };
    let dims = CheckedDims::new(img.width(), img.height(), 3).ok()?;
    if source.as_raw().len() != dims.total_bytes() {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    if let Some(output) = crate::color::ycbcr_luma_ssse3_bytes(source.as_raw()) {
        return Some(output);
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        let pixels = dims.total_pixels();
        if pixels == 0 {
            return Some(native_channel::gather_channel::<3, 0>(source.as_raw()));
        }
        let vector_pixels = pixels / 16 * 16;
        let input = source.as_raw();
        let mut output = Vec::<u8>::with_capacity(pixels);
        // SAFETY: The input length was checked against `dims.total_bytes()`
        // above. Each structure load reads 48 bytes from a complete sixteen-
        // pixel RGB block, and each store initializes sixteen reserved output
        // bytes. `set_len` runs after every prefix byte is initialized; the
        // remaining complete triples are appended below.
        #[allow(unsafe_code)]
        unsafe {
            use core::arch::aarch64 as neon;

            for pixel in (0..vector_pixels).step_by(16) {
                let rgb = neon::vld3q_u8(input.as_ptr().add(pixel * 3));
                neon::vst1q_u8(output.as_mut_ptr().add(pixel), rgb.0);
            }
            output.set_len(vector_pixels);
        }
        output.extend(
            input[vector_pixels * 3..]
                .chunks_exact(3)
                .map(|pixel| pixel[0]),
        );
        return Some((
            output,
            (vector_pixels / 16) as u64,
            (pixels - vector_pixels) as u64,
        ));
    }
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    Some(native_channel::gather_channel::<3, 0>(source.as_raw()))
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_ycbcr_grayscale_path() -> &'static str {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        "neon-deinterleave"
    }
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("ssse3") {
            "ssse3-deinterleave"
        } else {
            "vector-gather"
        }
    }
    #[cfg(not(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        target_arch = "x86_64"
    )))]
    {
        "vector-gather"
    }
}
