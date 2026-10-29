//! Native CPU execution and Pillow mode handling for ImageOps.grayscale.

use crate::checked_dims::CheckedDims;
use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::raster::DynamicImage;

/// Grayscale: convert to L-mode using Pillow's source-mode conversion path.
///
/// `ImageOps.grayscale` delegates to `Image.convert("L")`, whose C dispatch
/// does not reinterpret every source buffer as RGB. In particular, `I` and
/// `F` contain one scalar sample in a four-byte transport, `1` treats every
/// non-zero sample as white, and YCbCr copies its Y band directly. Keeping
/// those source semantics here is required before a later operation receives
/// the new L-mode segment.
pub(crate) fn op_grayscale(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let gray = match mode {
        Some("La") => {
            return Err(PilError::ValueError(
                "conversion from La to L not supported".into(),
            ));
        }
        // RGBa's fallback converter first restores RGB with integer division;
        // alpha zero preserves the stored color and over-alpha samples clip.
        Some("RGBa") => pil_grayscale(&crate::ops::pil_resize::unpremultiply_alpha(img))?,
        Some("I") => match crate::color::i_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("i_to_l always returns L mode"),
        },
        Some("F") => match crate::color::f_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("f_to_l always returns L mode"),
        },
        Some("1") => {
            let mut gray = img.to_luma8();
            for pixel in gray.pixels_mut() {
                pixel[0] = if pixel[0] == 0 { 0 } else { u8::MAX };
            }
            gray
        }
        Some("CMYK") => crate::color::cmyk_to_grayscale(img)?,
        Some("HSV") => crate::color::pil_grayscale(&crate::color::hsv_to_rgb(img))?,
        Some("YCbCr") => {
            // Pillow's Convert.c maps YCbCr→L through the Y band directly,
            // not through an RGB round trip and a second luma calculation.
            native_ycbcr_luma(img).unwrap_or_else(|| {
                let source = img.to_rgb8();
                crate::raster::GrayImage::from_fn(source.width(), source.height(), |x, y| {
                    crate::raster::Luma([source.get_pixel(x, y)[0]])
                })
            })
        }
        _ => pil_grayscale(img)?,
    };
    Ok(DynamicImage::ImageLuma8(gray))
}

/// Copy Y directly from an exact native YCbCr byte triple without first
/// cloning the full three-channel image into an RGB buffer. Empty or
/// mismatched storage retains the general conversion path above.
fn native_ycbcr_luma(img: &DynamicImage) -> Option<crate::raster::GrayImage> {
    let DynamicImage::ImageRgb8(source) = img else {
        return None;
    };
    let dims = CheckedDims::new(img.width(), img.height(), 3).ok()?;
    let pixels = dims.total_pixels();
    if pixels == 0 || source.as_raw().len() != dims.total_bytes() {
        return None;
    }
    // Keep CPU execution serial and portable. SSSE3 belongs to the explicit
    // SIMD family kernel; this bounded word path handles complete groups and
    // its scalar tail without changing source ownership or materialization.
    let output = native_ycbcr_luma_packed_blocks(source.as_raw(), pixels);
    crate::compute::record_pipeline_allocation(pixels);
    crate::raster::GrayImage::from_raw(img.width(), img.height(), output)
}

/// Extract four Y samples per block using bounded word loads and stores. Each
/// complete block is exactly twelve bytes, so the loads stay within the source
/// allocation. The remainder contains complete three-byte pixels only.
fn native_ycbcr_luma_packed_blocks(source: &[u8], pixels: usize) -> Vec<u8> {
    let mut packed = Vec::<[u8; 4]>::with_capacity(pixels.div_ceil(4));
    let mut blocks = source.chunks_exact(12);
    packed.extend(blocks.by_ref().map(|block| {
        let first = u32::from_le_bytes(
            block[0..4]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        let second = u32::from_le_bytes(
            block[4..8]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        let third = u32::from_le_bytes(
            block[8..12]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        [
            first as u8,
            (first >> 24) as u8,
            (second >> 16) as u8,
            (third >> 8) as u8,
        ]
    }));
    let remainder = blocks.remainder();
    if !remainder.is_empty() {
        let mut tail = [0; 4];
        for (pixel, luma) in remainder.chunks_exact(3).zip(&mut tail) {
            *luma = pixel[0];
        }
        packed.push(tail);
    }
    // `[u8; 4]` has byte alignment, so flattening reuses this allocation.
    let mut output = packed.into_flattened();
    output.truncate(pixels);
    debug_assert_eq!(output.len(), pixels);
    output
}

#[cfg(test)]
mod grayscale_native_tests {
    use super::op_grayscale;
    use crate::raster::{DynamicImage, RgbImage, RgbaImage};

    #[test]
    fn grayscale_keeps_integer_and_float_scalar_conversion_semantics() {
        let integers = [-1i32, 0, 1, 254, 255, 256];
        let integer_bytes: Vec<u8> = integers
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let integer_image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(integers.len() as u32, 1, integer_bytes).expect("packed I samples"),
        );
        let DynamicImage::ImageLuma8(integer_luma) =
            op_grayscale(&integer_image, Some("I")).expect("I grayscale")
        else {
            panic!("I grayscale must return L storage");
        };
        assert_eq!(integer_luma.as_raw(), &[0, 0, 1, 254, 255, 255]);

        let floats = [-1.5f32, 0.0, 1.9, 254.9, 255.0, 256.0];
        let float_bytes: Vec<u8> = floats
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let float_image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(floats.len() as u32, 1, float_bytes).expect("packed F samples"),
        );
        let DynamicImage::ImageLuma8(float_luma) =
            op_grayscale(&float_image, Some("F")).expect("F grayscale")
        else {
            panic!("F grayscale must return L storage");
        };
        assert_eq!(float_luma.as_raw(), &[0, 0, 1, 254, 255, 255]);
    }

    #[test]
    fn ycbcr_grayscale_copies_y_from_exact_native_triples() {
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(3, 1, vec![17, 240, 3, 129, 4, 250, 231, 78, 9])
                .expect("YCbCr byte triples"),
        );

        let result = op_grayscale(&image, Some("YCbCr")).expect("YCbCr grayscale");
        let DynamicImage::ImageLuma8(result) = result else {
            panic!("YCbCr grayscale must produce L storage");
        };
        assert_eq!(result.as_raw(), &[17, 129, 231]);
        assert_eq!(image.as_bytes(), &[17, 240, 3, 129, 4, 250, 231, 78, 9]);
    }

    #[test]
    fn ycbcr_grayscale_keeps_scalar_block_tails() {
        for pixels in 1..=8usize {
            let source: Vec<u8> = (0..pixels * 3)
                .map(|sample| (sample * 71 + 17) as u8)
                .collect();
            let expected: Vec<u8> = source.iter().step_by(3).copied().collect();
            assert_eq!(
                super::native_ycbcr_luma_packed_blocks(&source, pixels),
                expected,
                "packed block pixel count {pixels}"
            );
            let image = DynamicImage::ImageRgb8(
                RgbImage::from_raw(pixels as u32, 1, source).expect("YCbCr byte triples"),
            );

            let result = op_grayscale(&image, Some("YCbCr")).expect("YCbCr grayscale");
            let DynamicImage::ImageLuma8(result) = result else {
                panic!("YCbCr grayscale must produce L storage");
            };
            assert_eq!(result.as_raw(), &expected, "pixel count {pixels}");
        }
    }

    #[test]
    fn ycbcr_grayscale_keeps_empty_image_behavior() {
        let image = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
        let result = op_grayscale(&image, Some("YCbCr")).expect("empty YCbCr grayscale");
        let DynamicImage::ImageLuma8(result) = result else {
            panic!("empty YCbCr grayscale must produce L storage");
        };
        assert_eq!((result.width(), result.height()), (0, 1));
    }
}
