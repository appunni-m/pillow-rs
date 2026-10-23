use super::*;

#[cfg(all(test, not(feature = "parallel")))]
mod narrow_u8_resize_tests {
    use super::{
        FilterCoeffs, pil_resize, pil_resize_la_i32, pil_resize_luma_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage};

    #[test]
    fn hsv_narrow_cpu_resize_matches_wide_three_channel_resize() {
        for (width, height, output_width, output_height) in
            [(5, 3, 7, 5), (37, 17, 53, 23), (67, 29, 31, 41)]
        {
            let horizontal = precompute_coeffs(output_width, width, ResampleFilter::Bicubic);
            let vertical = precompute_coeffs(output_height, height, ResampleFilter::Bicubic);
            assert!(resize_u8_coefficients_fit_i32(&horizontal));
            assert!(resize_u8_coefficients_fit_i32(&vertical));

            for pattern in 0..4u32 {
                let bytes = (0..width * height * 3)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 if (index + index / (width * 3)) % 2 == 0 => 0,
                        2 => 255,
                        _ => index
                            .wrapping_mul(97)
                            .wrapping_add(index / (width * 3) * 53)
                            as u8,
                    })
                    .collect::<Vec<_>>();
                let image = DynamicImage::ImageRgb8(
                    RgbImage::from_raw(width, height, bytes).expect("HSV test image shape"),
                );
                let hsv = pil_resize(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    Some("HSV"),
                );
                let wide = pil_resize(
                    &image,
                    output_width,
                    output_height,
                    ResampleFilter::Bicubic,
                    Some("RGB"),
                );
                assert!(matches!(hsv, DynamicImage::ImageRgb8(_)));
                assert_eq!(
                    hsv.as_bytes(),
                    wide.as_bytes(),
                    "{width}x{height} pattern {pattern}"
                );
            }
        }
    }

    #[test]
    fn hsv_i32_accumulator_guard_fails_closed_at_the_overflow_boundary() {
        let safe = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_280],
        };
        let unsafe_sum = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![8_413_281],
        };
        let unsafe_weight = FilterCoeffs {
            xmin: vec![0],
            count: vec![1],
            offsets: vec![0],
            weights: vec![i64::MAX],
        };
        assert!(resize_u8_coefficients_fit_i32(&safe));
        assert!(!resize_u8_coefficients_fit_i32(&unsafe_sum));
        assert!(!resize_u8_coefficients_fit_i32(&unsafe_weight));
    }

    #[test]
    fn luma_narrow_cpu_resize_matches_wide_one_channel_reference() {
        for (width, height, output_width, output_height) in [
            (1, 7, 9, 3),
            (5, 3, 7, 5),
            (37, 17, 53, 23),
            (73, 41, 13, 9),
        ] {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .map(|index| match pattern {
                            0 => 0,
                            1 => 255,
                            2 if (index + index / width) % 2 == 0 => 0,
                            2 => 255,
                            _ => index
                                .wrapping_mul(73)
                                .wrapping_add(index / width * 19)
                                .wrapping_add(31) as u8,
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageLuma8(
                        GrayImage::from_raw(width, height, bytes).expect("L test image shape"),
                    );
                    let specialized = pil_resize_luma_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe L coefficients select the narrow path");

                    // A different logical-mode hint leaves the L storage at
                    // one byte per sample but bypasses the L-only candidate.
                    let reference =
                        pil_resize(&image, output_width, output_height, filter, Some("RGB"));
                    assert_eq!(
                        specialized,
                        reference.as_bytes(),
                        "{width}x{height}->{output_width}x{output_height} {filter:?} pattern {pattern}"
                    );
                }
            }
        }
    }

    #[test]
    fn la_narrow_cpu_resize_matches_wide_two_channel_reference() {
        let small_shapes = [
            (1, 7, 9, 3),
            (5, 3, 7, 5),
            (37, 17, 53, 23),
            (73, 41, 13, 9),
        ];
        for (width, height, output_width, output_height) in small_shapes {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .flat_map(|index| {
                            let luma = index
                                .wrapping_mul(73)
                                .wrapping_add(index / width * 19)
                                .wrapping_add(31) as u8;
                            let alpha = match pattern {
                                0 => 0,
                                1 => 255,
                                2 if (index + index / width) % 2 == 0 => 0,
                                2 => 255,
                                _ => index
                                    .wrapping_mul(47)
                                    .wrapping_add(index / width * 61)
                                    .wrapping_add(7) as u8,
                            };
                            [luma, alpha]
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageLumaA8(
                        GrayAlphaImage::from_raw(width, height, bytes)
                            .expect("LA test image shape"),
                    );
                    let specialized = pil_resize_la_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe LA coefficients select the narrow path");
                    let public = pil_resize(&image, output_width, output_height, filter, None);
                    let wide = pil_resize(&image, output_width, output_height, filter, Some("RGB"));
                    assert_eq!(specialized, wide.as_bytes());
                    assert_eq!(public.as_bytes(), wide.as_bytes());
                    assert!(matches!(public, DynamicImage::ImageLumaA8(_)));
                }
            }
        }

        // The generic path transposes at this size; the direct LA kernel
        // intentionally reads its row-major intermediate without that copy.
        let (width, height, output_width, output_height) = (641, 481, 853, 641);
        let bytes = (0u32..width * height)
            .flat_map(|index| {
                [
                    index.wrapping_mul(73).wrapping_add(23) as u8,
                    index.wrapping_mul(29).wrapping_add(11) as u8,
                ]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("large LA test image shape"),
        );
        let filter = ResampleFilter::Bicubic;
        let horizontal = precompute_coeffs(output_width, width, filter);
        let vertical = precompute_coeffs(output_height, height, filter);
        let specialized =
            pil_resize_la_i32(&image, output_width, output_height, &horizontal, &vertical)
                .expect("safe LA coefficients select the narrow path");
        let wide = pil_resize(&image, output_width, output_height, filter, Some("RGB"));
        assert_eq!(specialized, wide.as_bytes());
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod rgba_i32_resize_tests {
    use super::{
        horizontal_pass_rows_alpha, pil_resize, pil_resize_rgba_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32, vertical_pass_rows_alpha,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};

    fn wide_rgba_reference(
        image: &DynamicImage,
        output_width: u32,
        output_height: u32,
        horizontal: &super::FilterCoeffs,
        vertical: &super::FilterCoeffs,
    ) -> Vec<u8> {
        let source_height = image.height() as usize;
        let intermediate_stride = output_width as usize * 4;
        let mut intermediate = vec![0; source_height * intermediate_stride];
        horizontal_pass_rows_alpha(
            image.as_bytes(),
            image.width(),
            image.height(),
            4,
            horizontal,
            output_width,
            &mut intermediate,
        );
        let mut output = vec![0; output_height as usize * intermediate_stride];
        vertical_pass_rows_alpha(
            &intermediate,
            image.height(),
            output_width,
            output_height,
            4,
            vertical,
            &mut output,
            0,
        );
        output
    }

    #[test]
    fn rgba_narrow_cpu_resize_matches_wide_premultiplied_reference() {
        for (width, height, output_width, output_height) in
            [(5, 3, 7, 5), (37, 17, 53, 23), (73, 41, 13, 9)]
        {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, width, filter);
                let vertical = precompute_coeffs(output_height, height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..4u32 {
                    let bytes = (0..width * height)
                        .flat_map(|index| {
                            let alpha = match pattern {
                                0 => 0,
                                1 => 255,
                                2 if (index + index / width) % 2 == 0 => 0,
                                2 => 255,
                                _ => index
                                    .wrapping_mul(47)
                                    .wrapping_add(index / width * 61)
                                    .wrapping_add(7) as u8,
                            };
                            [
                                index.wrapping_mul(73).wrapping_add(31) as u8,
                                index.wrapping_mul(29).wrapping_add(19) as u8,
                                index.wrapping_mul(97).wrapping_add(11) as u8,
                                alpha,
                            ]
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(width, height, bytes).expect("RGBA test image shape"),
                    );
                    let specialized = pil_resize_rgba_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe RGBA coefficients select the narrow path");
                    let reference = wide_rgba_reference(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    );
                    assert_eq!(specialized, reference);

                    let public = pil_resize(&image, output_width, output_height, filter, None);
                    assert_eq!(public.as_bytes(), reference);
                    assert!(matches!(public, DynamicImage::ImageRgba8(_)));
                }
            }
        }
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod cmyk_i32_resize_tests {
    use super::{
        horizontal_pass_rows, pil_resize, pil_resize_cmyk_i32, precompute_coeffs,
        resize_u8_coefficients_fit_i32, vertical_pass_rows,
    };
    use crate::pipeline::ResampleFilter;
    use crate::raster::{DynamicImage, RgbaImage};

    fn wide_cmyk_reference(
        image: &DynamicImage,
        output_width: u32,
        output_height: u32,
        filter: ResampleFilter,
    ) -> Vec<u8> {
        let horizontal = precompute_coeffs(output_width, image.width(), filter);
        let vertical = precompute_coeffs(output_height, image.height(), filter);
        let intermediate_len = image.height() as usize * output_width as usize * 4;
        let mut intermediate = vec![0; intermediate_len];
        horizontal_pass_rows(
            image.as_bytes(),
            image.width(),
            image.height(),
            4,
            &horizontal,
            output_width,
            &mut intermediate,
        );
        let mut output = vec![0; output_width as usize * output_height as usize * 4];
        vertical_pass_rows(
            &intermediate,
            image.height(),
            output_width,
            output_height,
            4,
            &vertical,
            &mut output,
            0,
        );
        output
    }

    #[test]
    fn cmyk_narrow_resize_matches_wide_channel_arithmetic() {
        for (source_width, source_height, output_width, output_height) in
            [(7, 5, 11, 9), (11, 13, 5, 7), (1, 7, 9, 4), (9, 1, 4, 9)]
        {
            for filter in [
                ResampleFilter::Bilinear,
                ResampleFilter::Bicubic,
                ResampleFilter::Lanczos,
                ResampleFilter::Hamming,
                ResampleFilter::Box,
            ] {
                let horizontal = precompute_coeffs(output_width, source_width, filter);
                let vertical = precompute_coeffs(output_height, source_height, filter);
                assert!(resize_u8_coefficients_fit_i32(&horizontal));
                assert!(resize_u8_coefficients_fit_i32(&vertical));

                for pattern in 0..3u32 {
                    let samples = (0..source_width * source_height)
                        .flat_map(|pixel| {
                            let values = match pattern {
                                0 => [
                                    pixel.wrapping_mul(73).wrapping_add(19),
                                    pixel.wrapping_mul(41).wrapping_add(53),
                                    pixel.wrapping_mul(29).wrapping_add(97),
                                    pixel.wrapping_mul(11).wrapping_add(151),
                                ],
                                1 if pixel % 2 == 0 => [0; 4],
                                1 => [u32::MAX; 4],
                                _ => [0, u32::MAX, pixel * 17, pixel * 31],
                            };
                            values.map(|value| value as u8)
                        })
                        .collect::<Vec<_>>();
                    let image = DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(source_width, source_height, samples)
                            .expect("CMYK carrier dimensions are valid"),
                    );
                    let expected = wide_cmyk_reference(&image, output_width, output_height, filter);
                    let narrow = pil_resize_cmyk_i32(
                        &image,
                        output_width,
                        output_height,
                        &horizontal,
                        &vertical,
                    )
                    .expect("safe CMYK coefficients select the narrow path");
                    assert_eq!(narrow.as_bytes(), expected);
                    let actual =
                        pil_resize(&image, output_width, output_height, filter, Some("CMYK"));
                    assert_eq!(actual.as_bytes(), expected);
                    assert_eq!(actual.as_bytes(), narrow.as_bytes());
                    assert!(matches!(actual, DynamicImage::ImageRgba8(_)));
                }
            }
        }
    }

    #[test]
    fn cmyk_narrow_row_major_vertical_matches_transposed_wide_path() {
        let (source_width, source_height, output_width, output_height) =
            (1024u32, 768u32, 1365u32, 1024u32);
        let samples = (0..source_width * source_height)
            .flat_map(|pixel| {
                [
                    pixel.wrapping_mul(73).wrapping_add(19) as u8,
                    pixel.wrapping_mul(41).wrapping_add(53) as u8,
                    pixel.wrapping_mul(29).wrapping_add(97) as u8,
                    pixel.wrapping_mul(11).wrapping_add(151) as u8,
                ]
            })
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, samples)
                .expect("CMYK carrier dimensions are valid"),
        );
        let horizontal = precompute_coeffs(output_width, source_width, ResampleFilter::Bicubic);
        let vertical = precompute_coeffs(output_height, source_height, ResampleFilter::Bicubic);
        let expected =
            wide_cmyk_reference(&image, output_width, output_height, ResampleFilter::Bicubic);
        let actual =
            pil_resize_cmyk_i32(&image, output_width, output_height, &horizontal, &vertical)
                .expect("safe CMYK coefficients select the narrow path");
        assert_eq!(actual.as_bytes(), expected);
    }
}
