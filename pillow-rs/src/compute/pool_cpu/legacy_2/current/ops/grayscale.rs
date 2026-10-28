//! Native CPU execution and Pillow mode handling for ImageOps.grayscale.

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::raster::DynamicImage;

#[path = "grayscale/kernels/ycbcr.rs"]
mod ycbcr;

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
            ycbcr::luma(img).unwrap_or_else(|| {
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
                super::ycbcr::packed_luma_blocks(&source, pixels),
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
