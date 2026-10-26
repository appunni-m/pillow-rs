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
    if let Some(output) = super::ycbcr_ssse3::ycbcr_luma_ssse3_bytes(source.as_raw()) {
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
        // bytes. The four-block loop advances by 64 pixels and the remainder
        // loop handles each complete sixteen-pixel block once. `set_len` runs
        // after every prefix byte is initialized; complete triples left after
        // the vector prefix are appended below.
        #[allow(unsafe_code)]
        unsafe {
            use core::arch::aarch64 as neon;

            macro_rules! extract_y_block {
                ($pixel:expr) => {{
                    let rgb = neon::vld3q_u8(input.as_ptr().add($pixel * 3));
                    neon::vst1q_u8(output.as_mut_ptr().add($pixel), rgb.0);
                }};
            }

            let unrolled_pixels = vector_pixels / 64 * 64;
            for pixel in (0..unrolled_pixels).step_by(64) {
                extract_y_block!(pixel);
                extract_y_block!(pixel + 16);
                extract_y_block!(pixel + 32);
                extract_y_block!(pixel + 48);
            }
            for pixel in (unrolled_pixels..vector_pixels).step_by(16) {
                extract_y_block!(pixel);
            }
            output.set_len(vector_pixels);
        }
        output.extend(
            input[vector_pixels * 3..]
                .chunks_exact(3)
                .map(|pixel| pixel[0]),
        );
        Some((
            output,
            (vector_pixels / 16) as u64,
            (pixels - vector_pixels) as u64,
        ))
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

#[cfg(test)]
mod tests {
    use super::native_ycbcr_grayscale_bytes;
    use crate::raster::{DynamicImage, RgbImage};

    #[test]
    fn native_ycbcr_grayscale_handles_unrolled_vector_boundaries() {
        for pixels in [15usize, 16, 17, 63, 64, 65, 79, 80] {
            let mut source = Vec::with_capacity(pixels * 3);
            let mut expected = Vec::with_capacity(pixels);
            for pixel in 0..pixels {
                let y = (pixel * 37 + 11) as u8;
                source.extend_from_slice(&[y, 0xA5, 0x5A]);
                expected.push(y);
            }
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(pixels as u32, 1, source).expect("YCbCr triples"),
            );
            let (actual, vector_blocks, scalar_tail) =
                native_ycbcr_grayscale_bytes(&image).expect("native YCbCr extraction");

            assert_eq!(actual, expected, "pixel count {pixels}");
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            {
                let vector_pixels = pixels / 16 * 16;
                assert_eq!(
                    vector_blocks,
                    (vector_pixels / 16) as u64,
                    "pixel count {pixels}"
                );
                assert_eq!(
                    scalar_tail,
                    (pixels - vector_pixels) as u64,
                    "pixel count {pixels}"
                );
            }
            #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
            let _ = (vector_blocks, scalar_tail);
        }
    }
}
