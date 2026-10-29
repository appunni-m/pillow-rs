//! Focused contract tests for SIMD Invert effects.

#[test]
fn native_invert_writes_owned_p_indices_in_one_vector_pass() {
    use crate::raster::{DynamicImage, GrayImage};

    for width in [1u32, 15, 16, 17, 31] {
        let height = 3;
        let source = (0..width * height)
            .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let image =
            DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, source.clone()).unwrap());
        let inverted = super::native_invert(&image, Some("P"), false).unwrap();
        let expected = source.iter().map(|value| 255 - value).collect::<Vec<_>>();

        assert!(matches!(inverted, DynamicImage::ImageLuma8(_)));
        assert_eq!(inverted.as_bytes(), expected);
        assert_eq!(image.as_bytes(), source, "input remains immutable");
    }
}
