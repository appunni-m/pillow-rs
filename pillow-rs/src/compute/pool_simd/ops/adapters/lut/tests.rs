//! Focused contract tests for SIMD native LUT operations.

#[test]
fn native_lut_preserves_channel_order_and_vector_tails() {
    for channels in 1..=4 {
        let lut: Vec<u8> = (0..channels)
            .flat_map(|channel| {
                (0..256).map(move |value| ((value * 73 + channel * 47 + 19) & 255) as u8)
            })
            .collect();
        for width in [1, 15, 16, 17, 31, 32, 33, 257] {
            for height in [1, 3, 1025] {
                let mut bytes: Vec<u8> = (0..width * height * channels)
                    .map(|index| ((index * 113 + index / 7) & 255) as u8)
                    .collect();
                let expected: Vec<u8> = bytes
                    .iter()
                    .enumerate()
                    .map(|(index, &value)| lut[(index % channels) * 256 + usize::from(value)])
                    .collect();
                let original = bytes.clone();
                let tables = super::native_lut_tables_for_channels(&lut, channels)
                    .expect("valid native byte lookup tables");
                let mut mapped = vec![0u8; bytes.len()];
                super::native_lut_map_rows(
                    &bytes,
                    &mut mapped,
                    width,
                    height,
                    channels,
                    &lut,
                    &tables,
                )
                .expect("valid out-of-place native byte lookup");
                assert_eq!(
                    mapped, expected,
                    "out of place {width}x{height}, {channels}"
                );
                assert_eq!(
                    bytes, original,
                    "source mutated {width}x{height}, {channels}"
                );
                let (_, scalar_tail) =
                    super::native_lut_apply(&mut bytes, width, height, channels, &lut)
                        .expect("valid native byte lookup");
                assert_eq!(bytes, expected, "{width}x{height}, {channels} channels");
                assert_eq!(scalar_tail, 0);
            }
        }
    }
}

#[test]
fn native_rgb_affine_point_lut_matches_table_math_across_tails() {
    use crate::raster::{DynamicImage, RgbImage};

    let coefficients = [(3u8, 5u8), (5, 13), (255, 255)];
    let lut = coefficients
        .iter()
        .flat_map(|&(multiplier, offset)| {
            (0..=u8::MAX).map(move |sample| sample.wrapping_mul(multiplier).wrapping_add(offset))
        })
        .collect::<Vec<_>>();

    for width in [1u32, 15, 16, 17, 31, 32, 33, 257] {
        let height = 3u32;
        let source = (0..width * height * 3)
            .map(|index| index.wrapping_mul(73).wrapping_add(index / 7) as u8)
            .collect::<Vec<_>>();
        let expected = source
            .chunks_exact(3)
            .flat_map(|pixel| {
                [
                    lut[usize::from(pixel[0])],
                    lut[256 + usize::from(pixel[1])],
                    lut[512 + usize::from(pixel[2])],
                ]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source.clone()).expect("valid RGB source"),
        );
        let result =
            super::native_point_lut(&image, Some("RGB"), &lut).expect("affine RGB point LUT");

        assert_eq!(result.as_bytes(), expected, "{width}x{height}");
        assert_eq!(image.as_bytes(), source, "source remains unchanged");
    }
}
