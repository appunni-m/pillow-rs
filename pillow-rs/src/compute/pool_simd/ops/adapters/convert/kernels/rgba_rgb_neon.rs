//! AArch64 NEON kernel for native RGBA-to-RGB layout conversion.

use crate::raster::DynamicImage;

/// Drop the alpha byte from native RGBA pixels with sixteen-pixel NEON blocks.
///
/// A structure load and store handles sixteen pixels at once while preserving
/// the RGB sample bytes exactly.
#[allow(unsafe_code)]
pub(super) fn native_rgba_to_rgb_bytes(img: &DynamicImage) -> Option<(Vec<u8>, u64, u64)> {
    let DynamicImage::ImageRgba8(rgba) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source_bytes = pixels.checked_mul(4)?;
    let output_bytes = pixels.checked_mul(3)?;
    if output_bytes > isize::MAX as usize || rgba.as_raw().len() != source_bytes {
        return None;
    }

    let source = rgba.as_raw();
    let mut output = Vec::<u8>::new();
    output.try_reserve_exact(output_bytes).ok()?;
    let vector_pixels = pixels / 16 * 16;

    // SAFETY: dimensions prove the source contains exactly four initialized
    // bytes per pixel, and the exact output capacity was reserved above. Each
    // sixteen-pixel structure load reads 64 source bytes and each structure
    // store writes 48 destination bytes. The loop stays within vector_pixels;
    // the initialized prefix is exposed only after all complete blocks have
    // been written. The scalar tail appends the remaining complete pixels.
    unsafe {
        use core::arch::aarch64 as neon;

        for pixel in (0..vector_pixels).step_by(16) {
            let rgba_block = neon::vld4q_u8(source.as_ptr().add(pixel * 4));
            let rgb_block = neon::uint8x16x3_t(rgba_block.0, rgba_block.1, rgba_block.2);
            neon::vst3q_u8(output.as_mut_ptr().add(pixel * 3), rgb_block);
        }
        output.set_len(vector_pixels * 3);
    }

    for pixel in vector_pixels..pixels {
        let source_start = pixel * 4;
        output.extend_from_slice(&source[source_start..source_start + 3]);
    }

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
    fn rgba_to_rgb_neon_matches_cpu_at_vector_boundaries_and_tails() {
        for pixels in [1usize, 7, 15, 16, 17, 31, 32, 33, 71, 1025] {
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
                native_rgba_to_rgb_bytes(&image).expect("native RGBA to RGB conversion");

            assert_eq!(actual, expected.as_bytes(), "pixel count {pixels}");
            assert_eq!(vector_blocks, (pixels / 16) as u64, "pixel count {pixels}");
            assert_eq!(scalar_tail, (pixels % 16) as u64, "pixel count {pixels}");
        }
    }
}
