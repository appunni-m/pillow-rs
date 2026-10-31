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
