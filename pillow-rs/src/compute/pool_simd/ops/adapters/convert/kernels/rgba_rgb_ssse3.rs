//! x86 SSSE3 kernel for native RGBA-to-RGB layout conversion.

use crate::raster::DynamicImage;

/// Drop alpha from sixteen RGBA pixels and pack their RGB bytes directly into
/// the final output buffer. SSSE3 is checked before entering the target-feature
/// function; unsupported CPUs retain the generic vector conversion.
#[allow(unsafe_code)]
pub(super) fn native_rgba_to_rgb_bytes(img: &DynamicImage) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source_bytes = pixels.checked_mul(4)?;
    let output_bytes = pixels.checked_mul(3)?;
    if output_bytes > isize::MAX as usize
        || rgba.as_raw().len() != source_bytes
        || !std::is_x86_feature_detected!("ssse3")
    {
        return None;
    }

    // SAFETY: the layout and lengths are checked above, and runtime detection
    // confirms SSSE3. The target-feature function writes only complete
    // sixteen-pixel output blocks before exposing their initialized prefix.
    unsafe { rgba_to_rgb_ssse3(rgba.as_raw(), pixels, output_bytes) }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
#[allow(unsafe_code)]
unsafe fn rgba_to_rgb_ssse3(
    source: &[u8],
    pixels: usize,
    output_bytes: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    use core::arch::x86_64::{
        _mm_and_si128, _mm_loadu_si128, _mm_or_si128, _mm_setr_epi8, _mm_shuffle_epi8,
        _mm_slli_si128, _mm_srli_si128, _mm_storeu_si128,
    };

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::new();
    // Return to the generic kernel if the final buffer cannot be reserved.
    if output.try_reserve_exact(output_bytes).is_err() {
        return None;
    }

    // pshufb keeps the twelve RGB bytes from a four-pixel input block and
    // zeros its last four lanes. The masks combine four such blocks into
    // three contiguous sixteen-byte output stores per sixteen source pixels.
    unsafe {
        let rgb_lanes = _mm_setr_epi8(0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14, -1, -1, -1, -1);
        let low_twelve = _mm_setr_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 0, 0, 0);

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

            let output_start = output.as_mut_ptr().add(pixel * 3);
            let packed0 = _mm_or_si128(_mm_and_si128(rgb0, low_twelve), _mm_slli_si128::<12>(rgb1));
            let packed1 = _mm_or_si128(_mm_srli_si128::<4>(rgb1), _mm_slli_si128::<8>(rgb2));
            let packed2 = _mm_or_si128(_mm_srli_si128::<8>(rgb2), _mm_slli_si128::<4>(rgb3));
            _mm_storeu_si128(output_start.cast(), packed0);
            _mm_storeu_si128(output_start.add(16).cast(), packed1);
            _mm_storeu_si128(output_start.add(32).cast(), packed2);
        }
        output.set_len(vector_pixels * 3);
    }

    for pixel in vector_pixels..pixels {
        let source_start = pixel * 4;
        output.extend_from_slice(&source[source_start..source_start + 3]);
    }
    debug_assert_eq!(output.len(), output_bytes);
    Some((
        output,
        (vector_pixels / 16) as u64,
        (pixels - vector_pixels) as u64,
    ))
}

#[cfg(test)]
mod tests {
    use super::native_rgba_to_rgb_bytes;
    use crate::compute::pool_cpu::ops::color::op_convert;
    use crate::pipeline::ColorMode;
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn rgba_to_rgb_ssse3_matches_cpu_at_vector_boundaries_and_tails() {
        if !std::is_x86_feature_detected!("ssse3") {
            return;
        }
        for pixels in [1usize, 3, 4, 15, 16, 17, 31, 32, 33, 1023, 1024, 1025] {
            let source: Vec<u8> = (0..pixels * 4)
                .map(|byte| ((byte * 73 + 19) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageRgba8(
                RgbaImage::from_raw(pixels as u32, 1, source)
                    .expect("RGBA source has four bytes per pixel"),
            );
            let expected = op_convert(&image, &ColorMode::RGB, Some("RGBA"))
                .expect("CPU RGBA to RGB conversion");
            let (actual, vector_blocks, scalar_tail) =
                native_rgba_to_rgb_bytes(&image).expect("SSSE3 RGBA to RGB conversion");

            assert_eq!(actual, expected.as_bytes(), "pixel count {pixels}");
            assert_eq!(vector_blocks, (pixels / 16) as u64, "pixel count {pixels}");
            assert_eq!(scalar_tail, (pixels % 16) as u64, "pixel count {pixels}");
        }
    }
}
