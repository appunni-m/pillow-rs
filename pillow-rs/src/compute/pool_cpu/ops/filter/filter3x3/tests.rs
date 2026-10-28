#[cfg(test)]
mod i32_filter3x3_uniform_tests {
    use super::super::{
        I32_UNIFORM_FILTER3X3_MAX_PIXELS, execute_filter3x3, uniform_i32_filter3x3_is_identity,
    };
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn uniform_cross_smoothing_filter_returns_the_exact_input() {
        let (width, height, sample) = (32, 24, 37i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let kernel = [0.0, 1.0, 0.0, 1.0, 4.0, 1.0, 0.0, 1.0, 0.0];
        let normalized = std::array::from_fn(|index| kernel[index] / 8.0);
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &normalized,
            0.5,
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );
        let result = execute_filter3x3(&image, &kernel, 8.0, 0.0, Some("I")).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniformity_proof_rejects_nonidentity_nonuniform_and_oversized_inputs() {
        let (width, height, sample) = (7, 6, 11i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let mut identity_kernel = [0.0; 9];
        identity_kernel[4] = 1.0;
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &identity_kernel,
            0.5,
        ));

        let mut nonidentity_kernel = identity_kernel;
        nonidentity_kernel[4] = 2.0;
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &nonidentity_kernel,
            0.5,
        ));

        let mut varied = raw.clone();
        varied[4 * (width + 3)..4 * (width + 3) + 4].copy_from_slice(&12i32.to_le_bytes());
        assert!(!uniform_i32_filter3x3_is_identity(
            &varied,
            width,
            height,
            &identity_kernel,
            0.5,
        ));
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            I32_UNIFORM_FILTER3X3_MAX_PIXELS + 1,
            3,
            &identity_kernel,
            0.5,
        ));
    }
}

#[cfg(test)]
mod binomial3x3_native_la_tests {
    use super::super::{
        binomial3x3_parameters_match, filter_3x3_binomial_la_rows, filter_3x3_byte_rows,
    };

    #[test]
    fn native_la_ring_matches_ordered_float_kernel_and_copied_borders() {
        let kernel = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
        assert!(binomial3x3_parameters_match(&kernel, 16.0, 0.0));
        assert!(!binomial3x3_parameters_match(&kernel, 15.0, 0.0));
        assert!(!binomial3x3_parameters_match(&kernel, 16.0, 1.0));

        let normalized = std::array::from_fn(|index| kernel[index] / 16.0);
        for (width, height) in [
            (1usize, 1usize),
            (2, 5),
            (3, 3),
            (4, 7),
            (7, 5),
            (9, 7),
            (17, 9),
            (33, 35),
        ] {
            let raw: Vec<u8> = (0..width * height * 2)
                .map(|index| ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8)
                .collect();
            let mut expected = raw.clone();
            filter_3x3_byte_rows(
                &raw,
                &mut expected,
                width as i32,
                height as i32,
                2,
                &normalized,
                0.5,
            );

            let mut actual = raw.clone();
            assert!(filter_3x3_binomial_la_rows(
                &raw,
                &mut actual,
                width,
                height,
            ));
            assert_eq!(actual, expected, "native LA {width}x{height}");
        }
    }
}
