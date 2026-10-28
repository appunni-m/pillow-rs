#[cfg(test)]
mod binomial5x5_native_byte_tests {
    use super::super::{
        filter_5x5_binomial_la_rows, filter_5x5_binomial_luma_rows, filter_5x5_byte_rows,
    };

    #[test]
    fn integer_separable_path_matches_ordered_float_kernel_and_copied_borders() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [(8, 6), (9, 7), (5, 5), (4, 8), (8, 4)] {
            let raw: Vec<u8> = (0..width * height)
                .map(|index| ((index * 73 + index / width * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                1,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_5x5_binomial_luma_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "dimension {width}x{height}");
        }
    }

    #[test]
    fn native_la_ring_matches_ordered_float_kernel_for_each_interleaved_band() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (5, 5),
            (7, 5),
            (9, 7),
            (17, 9),
            (33, 35),
        ] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_5x5_binomial_la_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }

    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    #[test]
    fn native_la_vector_ring_matches_ordered_float_kernel_across_vector_tails() {
        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel: [f32; 25] =
            std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        for (width, height) in [(5usize, 5usize), (13, 7), (19, 11), (20, 13), (31, 17)] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            let (vector_blocks, scalar_tail) =
                crate::compute::pool_simd::ops::native_filter::native_filter_5x5_binomial_la_rows(
                    &raw,
                    &mut actual,
                    width,
                    height,
                )
                .expect("valid native LA vector dimensions");
            if width >= 13 && height >= 5 {
                assert!(vector_blocks > 0, "{width}x{height} should vectorize");
            }
            if width == 13 || width == 19 || width == 31 {
                assert!(scalar_tail > 0, "{width}x{height} should have a tail");
            }
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }
}

#[cfg(test)]
mod la_single_outlier5x5_tests {
    use super::super::{
        filter_5x5_byte_rows, filter_5x5_la_single_outlier_rows,
        la_single_outlier5x5_parameters_match,
    };

    #[test]
    fn rolling_integer_sums_match_ordered_float_for_la_pixels_and_borders() {
        let mut kernel = [1.0f32; 25];
        kernel[24] = 2.0;
        let normalized = std::array::from_fn(|index| kernel[index] / 25.0);
        assert!(la_single_outlier5x5_parameters_match(&kernel, 25.0, 0.0));
        assert!(!la_single_outlier5x5_parameters_match(&kernel, 24.0, 0.0));
        assert!(!la_single_outlier5x5_parameters_match(&kernel, 25.0, 1.0));

        let mut misplaced_outlier = kernel;
        misplaced_outlier[23] = 2.0;
        misplaced_outlier[24] = 1.0;
        assert!(!la_single_outlier5x5_parameters_match(
            &misplaced_outlier,
            25.0,
            0.0
        ));

        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (7, 4),
            (5, 5),
            (7, 5),
            (13, 7),
            (19, 11),
            (71, 67),
        ] {
            for pattern in 0..4 {
                let mut state = 0x9e37_79b9u32;
                let raw: Vec<u8> = (0..width * height * 2)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 => ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8,
                        _ => {
                            state ^= state << 13;
                            state ^= state >> 17;
                            state ^= state << 5;
                            state as u8
                        }
                    })
                    .collect();
                let mut expected = raw.clone();
                filter_5x5_byte_rows(
                    &raw,
                    &mut expected,
                    width as i32,
                    height as i32,
                    2,
                    &normalized,
                    0.5,
                );

                let mut actual = raw.clone();
                assert!(filter_5x5_la_single_outlier_rows(
                    &raw,
                    &mut actual,
                    width,
                    height,
                ));
                assert_eq!(
                    actual, expected,
                    "native LA {width}x{height}, pattern {pattern}"
                );
            }
        }
    }
}

#[cfg(test)]
mod uniform5x5_native_luma_tests {
    use super::super::{
        filter_5x5_byte_rows, filter_5x5_uniform_luma_rows, uniform5x5_parameters_match,
    };

    #[test]
    fn rolling_integer_rows_match_ordered_float_for_all_sums_and_borders() {
        let kernel = [1.0f32; 25];
        let normalized = [1.0f32 / 25.0; 25];
        assert!(uniform5x5_parameters_match(&kernel, 25.0, 0.0));
        assert!(!uniform5x5_parameters_match(&kernel, 24.0, 0.0));
        assert!(!uniform5x5_parameters_match(&kernel, 25.0, 1.0));
        let mut almost_uniform = kernel;
        almost_uniform[12] = f32::from_bits(1.0f32.to_bits() + 1);
        assert!(!uniform5x5_parameters_match(&almost_uniform, 25.0, 0.0));

        // Every integer numerator from 0 through 25*255 is representable by
        // 25 byte samples. Compare the integer path against Pillow's ordered
        // multiply/FMA/row-add path at every possible byte-rounding boundary.
        for sum in 0..=25 * 255 {
            let mut raw = [0u8; 25];
            let mut remaining = sum;
            for sample in &mut raw {
                let value = remaining.min(255);
                *sample = value as u8;
                remaining -= value;
            }
            let mut expected = raw;
            filter_5x5_byte_rows(&raw, &mut expected, 5, 5, 1, &normalized, 0.5);
            let mut actual = raw;
            assert!(filter_5x5_uniform_luma_rows(&raw, &mut actual, 5, 5));
            assert_eq!(actual[12], expected[12], "window sum {sum}");
        }

        for (width, height) in [(13usize, 7usize), (4, 7), (11, 13), (19, 11)] {
            let raw = (0..width * height)
                .map(|index| ((index * 73 + index / width * 29 + 17) % 256) as u8)
                .collect::<Vec<_>>();
            let mut expected = raw.clone();
            filter_5x5_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                1,
                &normalized,
                0.5,
            );
            let mut actual = raw.clone();
            assert!(filter_5x5_uniform_luma_rows(
                &raw,
                &mut actual,
                width,
                height
            ));
            assert_eq!(actual, expected, "{width}x{height}");
        }
    }
}

#[cfg(test)]
mod uniform5x5_native_rgba_tests {
    use super::super::{filter_5x5_byte_rows, filter_5x5_uniform_rgba_rows};

    #[test]
    fn rolling_integer_rows_match_ordered_float_for_odd_width_and_copied_borders() {
        let (width, height) = (67usize, 63usize);
        let raw = (0..width * height * 4)
            .map(|index| ((index * 73 + index / (width * 4) * 29 + 17) % 256) as u8)
            .collect::<Vec<_>>();
        let normalized = [1.0f32 / 25.0; 25];

        let mut expected = raw.clone();
        filter_5x5_byte_rows(
            &raw,
            &mut expected,
            width as i32,
            height as i32,
            4,
            &normalized,
            0.5,
        );

        let mut actual = raw.clone();
        assert!(filter_5x5_uniform_rgba_rows(
            &raw,
            &mut actual,
            width,
            height,
        ));
        assert_eq!(actual, expected, "native RGBA {width}x{height}");
    }
}
