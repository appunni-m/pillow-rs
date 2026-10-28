#[cfg(test)]
mod gaussian_blur_row_fusion_tests {
    use super::super::{
        blur_columns, blur_interleaved_rows_three_passes,
        blur_interleaved_vertical_radius_one_fractional_rows, blur_la_radius_one_fractional_line,
        blur_line, blur_luma_radius_one_fractional_line, blur_luma_rows_three_passes,
        blur_luma_vertical_radius_one_rows, blur_parameters, blur_rgb_radius_one_fractional_line,
        blur_rows, execute_gaussian_blur, gaussian_blur_box_radius,
        uniform_rgba_box_blur_is_identity,
    };
    use crate::raster::{DynamicImage, GrayAlphaImage, RgbaImage};

    fn three_pass_rgba_reference(
        source: &[u8],
        width: usize,
        height: usize,
        radius: f32,
    ) -> Vec<u8> {
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        let mut work = source.to_vec();
        let mut scratch = vec![0; source.len()];
        for _ in 0..3 {
            blur_rows(
                &work,
                &mut scratch,
                width,
                height,
                4,
                integer_radius,
                whole_weight,
                fractional_weight,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
        let mut accumulator = vec![0u32; width * 4];
        for _ in 0..3 {
            blur_columns(
                &work,
                &mut scratch,
                width,
                height,
                4,
                integer_radius,
                whole_weight,
                fractional_weight,
                &mut accumulator,
            );
            std::mem::swap(&mut work, &mut scratch);
        }
        work
    }

    #[test]
    fn large_uniform_rgba_gaussian_blur_returns_exact_input() {
        let (width, height) = (1024usize, 768usize);
        let sample = [83, 149, 17, 191];
        let raw = sample.repeat(width * height);
        let radius = gaussian_blur_box_radius(7.5);
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        assert_eq!(integer_radius, 7);
        assert_eq!(whole_weight, 1_116_425);
        assert_eq!(fractional_weight, 15_420);
        assert!(uniform_rgba_box_blur_is_identity(
            &raw,
            width,
            height,
            (integer_radius, whole_weight, fractional_weight),
            (integer_radius, whole_weight, fractional_weight),
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                .expect("valid uniform RGBA image"),
        );
        let actual = execute_gaussian_blur(&image, 7.5).expect("GaussianBlur");
        assert_eq!(actual.as_bytes(), raw);
    }

    #[test]
    fn large_rgba_gaussian_blur_full_scan_rejects_an_unsampled_pixel() {
        let (width, height) = (67usize, 67usize);
        let sample = [83, 149, 17, 191];
        let mut raw = sample.repeat(width * height);
        // This point is outside the 7-by-5 spaced rejection probes. The
        // complete scan must still find it and let the real blur run.
        raw[(width + 1) * 4..(width + 2) * 4].copy_from_slice(&[255, 0, 0, 0]);
        let radius = gaussian_blur_box_radius(7.5);
        let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
        assert!(!uniform_rgba_box_blur_is_identity(
            &raw,
            width,
            height,
            (integer_radius, whole_weight, fractional_weight),
            (integer_radius, whole_weight, fractional_weight),
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                .expect("valid adversarial RGBA image"),
        );
        let actual = execute_gaussian_blur(&image, 7.5).expect("GaussianBlur");
        let expected = three_pass_rgba_reference(&raw, width, height, radius);
        assert_eq!(actual.as_bytes(), expected);
        assert_ne!(actual.as_bytes(), raw);
    }

    #[test]
    fn two_channel_step_matches_direct_box_blur_reference() {
        for width in [1, 2, 5, 17] {
            let source = (0..width * 2)
                .map(|index| ((index * 47 + index / 2 * 31 + 17) % 256) as u8)
                .collect::<Vec<_>>();
            for radius in [0.0, 0.25, 1.0, 1.375, 4.75, 30.0] {
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
                let mut actual = vec![0u8; source.len()];
                let mut accumulator = [0u32; 2];
                blur_line(
                    &source,
                    &mut actual,
                    0,
                    width,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );

                let mut expected = vec![0u8; source.len()];
                let last = width as isize - 1;
                for x in 0..width {
                    for channel in 0..2 {
                        let mut whole_sum = 0u32;
                        for offset in -(integer_radius as isize)..=(integer_radius as isize) {
                            let sample_x = (x as isize + offset).clamp(0, last) as usize;
                            whole_sum =
                                whole_sum.wrapping_add(u32::from(source[sample_x * 2 + channel]));
                        }
                        let mut weighted_sum = whole_sum.wrapping_mul(whole_weight);
                        if fractional_weight != 0 {
                            let left_x =
                                (x as isize - integer_radius as isize - 1).clamp(0, last) as usize;
                            let right_x =
                                (x as isize + integer_radius as isize + 1).clamp(0, last) as usize;
                            let far_sum = u32::from(source[left_x * 2 + channel])
                                + u32::from(source[right_x * 2 + channel]);
                            weighted_sum =
                                weighted_sum.wrapping_add(far_sum.wrapping_mul(fractional_weight));
                        }
                        expected[x * 2 + channel] =
                            (weighted_sum.wrapping_add(super::super::BOX_BLUR_BIAS) >> 24) as u8;
                    }
                }

                assert_eq!(
                    actual, expected,
                    "width={width}, radius={radius}, integer_radius={integer_radius}"
                );
            }
        }
    }

    #[test]
    fn la_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [5, 6, 7, 17, 65, 1024] {
            let source = (0..width * 2)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; source.len()];
            let mut expected = vec![0u8; source.len()];
            blur_la_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                2,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "LA row width={width}");
        }
    }

    #[test]
    fn rgb_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [5, 6, 7, 17, 65, 1024] {
            let source = (0..width * 3)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; source.len()];
            let mut expected = vec![0u8; source.len()];
            blur_rgb_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                3,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "RGB row width={width}");
        }
    }

    #[test]
    fn luma_radius_one_fractional_line_matches_generic_fixed_point_blur() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for width in [1, 2, 3, 4, 5, 6, 7, 17, 65, 1024] {
            let source = (0..width)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; width];
            let mut expected = vec![0u8; width];
            blur_luma_radius_one_fractional_line(
                &source,
                &mut actual,
                width,
                whole_weight,
                fractional_weight,
            );
            blur_line(
                &source,
                &mut expected,
                0,
                width,
                1,
                radius,
                whole_weight,
                fractional_weight,
                &mut [0u32; 4],
            );

            assert_eq!(actual, expected, "L row width={width}");
        }
    }

    #[test]
    fn luma_three_pass_row_fusion_matches_full_frame_passes() {
        for (width, height, radius) in [
            (1, 1, 1.375),
            (2, 3, 1.375),
            (5, 7, 0.25),
            (17, 9, 1.375),
            (65, 47, 2.5),
        ] {
            let source = (0..width * height)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
            let mut fused = vec![0u8; source.len()];
            let mut reference_a = source.clone();
            let mut reference_b = vec![0u8; source.len()];

            blur_luma_rows_three_passes(
                &source,
                &mut fused,
                width,
                height,
                integer_radius,
                whole_weight,
                fractional_weight,
            );
            for _ in 0..3 {
                blur_rows(
                    &reference_a,
                    &mut reference_b,
                    width,
                    height,
                    1,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut reference_a, &mut reference_b);
            }

            assert_eq!(
                fused, reference_a,
                "L rows {width}x{height}, radius={radius}"
            );
        }
    }

    #[test]
    fn luma_vertical_radius_one_rows_match_fixed_point_column_reference() {
        for (width, height) in [(1, 1), (1, 7), (2, 3), (17, 9), (65, 47)] {
            for radius in [1.0, 1.375] {
                let source = (0..width * height)
                    .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0u8; source.len()];
                let mut expected = vec![0u8; source.len()];
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);

                blur_luma_vertical_radius_one_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    whole_weight,
                    fractional_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    1,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0u32; width],
                );

                assert_eq!(
                    actual, expected,
                    "L vertical {width}x{height}, radius={radius}"
                );
            }
        }
    }

    #[test]
    fn interleaved_vertical_radius_one_fractional_rows_match_fixed_point_reference() {
        let (radius, whole_weight, fractional_weight) = blur_parameters(1.375);
        assert_eq!(radius, 1);
        assert_ne!(fractional_weight, 0);

        for (width, height) in [(1, 1), (1, 7), (2, 3), (5, 4), (17, 9), (65, 47)] {
            for channels in [2, 3, 4] {
                let len = width * height * channels;
                let source = (0..len)
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let mut actual = vec![0u8; len];
                let mut expected = vec![0u8; len];

                blur_interleaved_vertical_radius_one_fractional_rows(
                    &source,
                    &mut actual,
                    width,
                    height,
                    channels,
                    whole_weight,
                    fractional_weight,
                );
                blur_columns(
                    &source,
                    &mut expected,
                    width,
                    height,
                    channels,
                    radius,
                    whole_weight,
                    fractional_weight,
                    &mut vec![0u32; width * channels],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }

    #[test]
    fn single_channel_step_matches_two_component_generic_reference() {
        for (width, height, radius) in [
            (1, 1, 1.375),
            (2, 3, 1.375),
            (5, 7, 0.25),
            (17, 9, 1.375),
            (65, 47, 2.5),
        ] {
            let source = (0..width * height)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let mut specialized = vec![0u8; source.len()];
            let mut generic = vec![0u8; source.len()];
            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);

            blur_rows(
                &source,
                &mut specialized,
                width,
                height,
                1,
                integer_radius,
                whole_weight,
                fractional_weight,
            );

            // A two-byte pixel takes the generic recurrence while each byte
            // evolves independently. Duplicate every L sample and use the
            // first output byte as the one-channel reference.
            for row in 0..height {
                let row_start = row * width;
                let mut paired_source = Vec::with_capacity(width * 2);
                for &sample in &source[row_start..row_start + width] {
                    paired_source.extend_from_slice(&[sample, sample]);
                }
                let mut paired_output = vec![0u8; width * 2];
                let mut accumulator = [0u32; 4];
                blur_line(
                    &paired_source,
                    &mut paired_output,
                    0,
                    width,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );
                for (x, pair) in paired_output.chunks_exact(2).enumerate() {
                    assert_eq!(pair[0], pair[1]);
                    generic[row_start + x] = pair[0];
                }
            }

            assert_eq!(
                specialized, generic,
                "L rows {width}x{height}, radius={radius}"
            );
        }
    }

    #[test]
    fn interleaved_gaussian_horizontal_row_fusion_matches_three_full_frame_passes() {
        for channels in [2, 3, 4] {
            for (width, height, radius) in
                [(1, 1, 1.375), (2, 3, 1.375), (7, 5, 1.375), (65, 47, 1.375)]
            {
                let pixel_count = width * height;
                let source = (0..pixel_count * channels)
                    .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                    .collect::<Vec<_>>();
                let (integer_radius, whole_weight, fractional_weight) = blur_parameters(radius);
                let mut fused = vec![0; source.len()];
                match channels {
                    2 => blur_interleaved_rows_three_passes::<2>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    3 => blur_interleaved_rows_three_passes::<3>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    4 => blur_interleaved_rows_three_passes::<4>(
                        &source,
                        &mut fused,
                        width,
                        height,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    ),
                    _ => unreachable!("test channels are 2, 3, or 4"),
                }

                let mut work = source.clone();
                let mut scratch = vec![0; source.len()];
                for _ in 0..3 {
                    blur_rows(
                        &work,
                        &mut scratch,
                        width,
                        height,
                        channels,
                        integer_radius,
                        whole_weight,
                        fractional_weight,
                    );
                    std::mem::swap(&mut work, &mut scratch);
                }

                assert_eq!(
                    fused, work,
                    "fused {channels}-channel passes differ at {width}×{height}"
                );
            }
        }
    }

    #[test]
    fn la_gaussian_blur_row_fusion_matches_full_frame_passes() {
        for (width, height) in [(1, 1), (2, 3), (7, 5), (65, 47)] {
            let pixel_count = width * height;
            let source = (0..pixel_count * 2)
                .map(|index| ((index * 73 + index / 7 * 19 + 31) % 256) as u8)
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width as u32, height as u32, source.clone())
                    .expect("valid LA fixture"),
            );
            let actual = execute_gaussian_blur(&image, 2.0).expect("LA GaussianBlur");

            let (integer_radius, whole_weight, fractional_weight) = blur_parameters(1.375);
            let mut work = source;
            let mut scratch = vec![0; work.len()];
            for _ in 0..3 {
                blur_rows(
                    &work,
                    &mut scratch,
                    width,
                    height,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut work, &mut scratch);
            }

            let mut accumulator = vec![0u32; width * 2];
            for _ in 0..3 {
                blur_columns(
                    &work,
                    &mut scratch,
                    width,
                    height,
                    2,
                    integer_radius,
                    whole_weight,
                    fractional_weight,
                    &mut accumulator,
                );
                std::mem::swap(&mut work, &mut scratch);
            }

            assert_eq!(
                actual.as_bytes(),
                work,
                "fused LA GaussianBlur differs at {width}×{height}"
            );
        }
    }
}
