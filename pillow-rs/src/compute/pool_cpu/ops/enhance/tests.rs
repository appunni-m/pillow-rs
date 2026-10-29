use crate::Image;

use super::ops::op_enhance_brightness;
use crate::image::Mode;
use crate::raster::{DynamicImage, GrayImage};

#[test]
fn luma_brightness_binary_factors_match_float_truncation_for_every_byte() {
    let input: Vec<u8> = (0..=u8::MAX).collect();
    let source = GrayImage::from_raw(input.len() as u32, 1, input.clone())
        .expect("valid one-row grayscale image");
    let source = DynamicImage::ImageLuma8(source);

    for factor in [0.5, 0.25, 0.125] {
        let result = op_enhance_brightness(&source, factor, Mode::ImageLuma8)
            .expect("brightness transform succeeds");
        let expected: Vec<u8> = input
            .iter()
            .map(|sample| (*sample as f64 * factor).clamp(0.0, 255.0) as u8)
            .collect();
        assert_eq!(result.as_bytes(), expected);
    }
}

#[test]
fn contrast_preserves_empty_cmyk_images() {
    for size in [(0, 0), (0, 3), (3, 0)] {
        for factor in [0.0, 0.5, 1.0, 2.0] {
            let image =
                Image::new(size.0, size.1, "CMYK", (0, 0, 0, 0)).expect("empty CMYK image");
            let enhanced = image
                .enhance_contrast(factor)
                .expect("Contrast queues for empty CMYK image");

            assert_eq!(enhanced.size().expect("empty size"), size);
            assert_eq!(enhanced.mode().expect("CMYK mode"), "CMYK");
            assert!(enhanced.tobytes().expect("empty bytes").is_empty());
        }
    }
}
