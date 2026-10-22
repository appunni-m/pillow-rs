//! Focused contract tests for SIMD ImageEnhance and point effects.

#[test]
fn la_brightness_binary_shifts_match_float_truncation_and_preserve_alpha() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayAlphaImage};

    for (width, height) in [(256u32, 256u32), (17, 3), (3, 17)] {
        let pixels = (width * height) as usize;
        let source = if pixels == 256 * 256 {
            (0..pixels)
                .flat_map(|index| [(index >> 8) as u8, index as u8])
                .collect::<Vec<_>>()
        } else {
            (0..pixels)
                .flat_map(|index| {
                    [
                        (index.wrapping_mul(71).wrapping_add(13)) as u8,
                        (index.wrapping_mul(43).wrapping_add(29)) as u8,
                    ]
                })
                .collect::<Vec<_>>()
        };
        let image = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, source.clone())
                .expect("LA brightness source shape"),
        );

        for (factor, shift) in [(0.5, 1), (0.25, 2), (0.125, 3)] {
            let operation = PipelineOp::Brightness { factor };
            let expected = source
                .chunks_exact(2)
                .flat_map(|pixel| [(pixel[0] >> shift), pixel[1]])
                .collect::<Vec<_>>();
            let direct = super::simd_brightness(&image, &operation, Some("LA"))
                .expect("direct SIMD LA brightness");
            assert!(matches!(direct, DynamicImage::ImageLumaA8(_)));
            assert_eq!(
                direct.as_bytes(),
                expected,
                "direct {factor} {width}x{height}"
            );

            let mut in_place = image.clone();
            assert!(
                super::super::simd_execute_in_place(&mut in_place, &operation, Some("LA"))
                    .expect("in-place SIMD LA brightness dispatch")
            );
            assert_eq!(
                in_place.as_bytes(),
                expected,
                "in-place {factor} {width}x{height}"
            );
            assert_eq!(image.as_bytes(), source, "source remains unchanged");
        }
    }
}

#[test]
fn native_rgb_sharpness_vector_stream_matches_cpu_across_tails() {
    use crate::pipeline::PipelineOp;
    use crate::raster::DynamicImage;
    for (width, height) in [(3, 3), (4, 5), (5, 7), (8, 4), (9, 9), (17, 6)] {
        let pixels = (width * height) as usize;
        let bytes = (0..pixels * 3)
            .map(|index| (index.wrapping_mul(61).wrapping_add(index / 7 + 23)) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgb8(
            crate::raster::RgbImage::from_raw(width, height, bytes).unwrap(),
        );
        for factor in [0.0, 0.5, 1.5, 2.0, f64::MAX] {
            let op = PipelineOp::Sharpness { factor };
            let expected =
                crate::compute::registry::execute_cpu(&op, &source, Some("RGB")).unwrap();
            let actual = super::simd_sharpness(&source, &op, Some("RGB")).unwrap();
            assert_eq!(
                actual.color(),
                expected.color(),
                "{width}x{height}, {factor}"
            );
            assert_eq!(
                actual.as_bytes(),
                expected.as_bytes(),
                "{width}x{height}, {factor}"
            );
        }
    }
}

#[test]
fn native_sharpness_integer_channels_match_cpu_for_all_byte_modes() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
    for (mode, channels) in [
        ("L", 1usize),
        ("LA", 2),
        ("RGB", 3),
        ("RGBA", 4),
        ("RGBX", 4),
        ("CMYK", 4),
    ] {
        for (width, height) in [
            (3, 3),
            (4, 5),
            (5, 7),
            (9, 4),
            (10, 9),
            (17, 6),
            (18, 5),
            (25, 8),
            (33, 4),
        ] {
            let pixels = width as usize * height as usize;
            let bytes = (0..pixels * channels)
                .map(|index| (index.wrapping_mul(61).wrapping_add(index / 7 + 23)) as u8)
                .collect::<Vec<_>>();
            let image = match channels {
                1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, bytes).unwrap()),
                2 => DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(width, height, bytes).unwrap(),
                ),
                3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, bytes).unwrap()),
                4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, bytes).unwrap()),
                _ => unreachable!(),
            };
            for factor in [0.0, 0.5, 1.5, 2.0, f64::MAX] {
                let operation = PipelineOp::Sharpness { factor };
                let expected =
                    crate::compute::registry::execute_cpu(&operation, &image, Some(mode)).unwrap();
                let actual = super::simd_sharpness(&image, &operation, Some(mode)).unwrap();
                assert_eq!(
                    actual.color(),
                    expected.color(),
                    "{mode} {width}x{height} {factor}"
                );
                assert_eq!(
                    actual.as_bytes(),
                    expected.as_bytes(),
                    "{mode} {width}x{height} {factor}"
                );
            }
        }
    }
}

#[test]
fn posterize_direct_copy_matches_all_bits_and_vector_tails() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage, RgbImage};
    for (width, height) in [(1, 1), (5, 1), (17, 3), (256, 1), (513, 512)] {
        let pixels = (width * height) as usize;
        let luma_bytes = (0..pixels)
            .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let rgb_bytes = (0..pixels * 3)
            .map(|index| (index.wrapping_mul(47).wrapping_add(31)) as u8)
            .collect::<Vec<_>>();
        let luma = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, luma_bytes.clone()).unwrap(),
        );
        let rgb =
            DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, rgb_bytes.clone()).unwrap());

        for bits in 1..=8u8 {
            let mask = !((1u8 << (8 - bits)) - 1);
            let operation = PipelineOp::Posterize { bits };
            let expected_luma = luma_bytes
                .iter()
                .map(|value| value & mask)
                .collect::<Vec<_>>();
            let expected_rgb = rgb_bytes
                .iter()
                .map(|value| value & mask)
                .collect::<Vec<_>>();
            let actual_luma = super::simd_posterize(&luma, &operation, Some("L")).unwrap();
            let actual_rgb = super::simd_posterize(&rgb, &operation, Some("RGB")).unwrap();

            assert!(matches!(actual_luma, DynamicImage::ImageLuma8(_)));
            assert!(matches!(actual_rgb, DynamicImage::ImageRgb8(_)));
            assert_eq!(actual_luma.as_bytes(), expected_luma);
            assert_eq!(actual_rgb.as_bytes(), expected_rgb);
            assert_eq!(luma.as_bytes(), luma_bytes);
            assert_eq!(rgb.as_bytes(), rgb_bytes);
        }
    }
}
