#[cfg(test)]
mod f_mode_rank_filter_uniform_tests {
    use super::{
        execute, native_uniform_f_mode_image, rank_filter_f_requires_legacy_sort,
        select_rank_filter_f_value,
    };
    use crate::raster::{DynamicImage, RgbaImage};

    fn raw_f32(values: &[f32]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(values.len() * 4);
        for value in values {
            raw.extend_from_slice(&value.to_le_bytes());
        }
        raw
    }

    #[test]
    fn uniform_f_mode_rank_filter_preserves_exact_source_bytes() {
        let (width, height, sample) = (256, 256, 37.0f32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let image = DynamicImage::ImageF32(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );

        let result = execute(&image, 9, 40).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniform_f_mode_detection_is_bit_exact() {
        assert!(native_uniform_f_mode_image(
            &37.0f32.to_le_bytes().repeat(4),
            2,
            2
        ));
        assert!(!native_uniform_f_mode_image(
            &[0.0f32.to_le_bytes(), (-0.0f32).to_le_bytes()].concat(),
            2,
            1,
        ));
        assert!(!native_uniform_f_mode_image(&37.0f32.to_le_bytes(), 0, 1));
    }

    #[test]
    fn f_mode_selection_matches_full_sort_for_finite_ties() {
        let original = [8.0, -3.0, 8.0, 0.5, 11.0, -3.0, 4.0, 1.0, 8.0];
        assert!(!rank_filter_f_requires_legacy_sort(&raw_f32(&original)));
        for rank in 0..original.len() {
            let mut sorted = original;
            sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let expected = sorted[rank].to_bits();
            let mut selected = original.to_vec();
            assert_eq!(
                select_rank_filter_f_value(&mut selected, rank, false).to_bits(),
                expected,
            );
        }
    }

    #[test]
    fn f_mode_non_total_values_keep_legacy_sort() {
        assert!(rank_filter_f_requires_legacy_sort(&raw_f32(&[
            1.0,
            f32::from_bits(0x7fc0_1234),
            2.0,
        ])));
        assert!(rank_filter_f_requires_legacy_sort(&raw_f32(&[
            -0.0, 1.0, 0.0
        ])));
        assert!(!rank_filter_f_requires_legacy_sort(&raw_f32(&[
            -0.0, -0.0, 1.0
        ])));
    }
}

