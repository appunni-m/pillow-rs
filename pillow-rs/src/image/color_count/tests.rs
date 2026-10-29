use super::{
    PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES, PillowColorCounts, RgbGrayscaleColorCounts,
    SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES, SCALAR_I_SPARSE_RUN_COLOR_LIMIT,
    ScalarColorDomainCounts, getcolors_rgb8_grayscale, getcolors_scalar_i_repeated_runs,
    pillow_color_components, pillow_color_direct_table_slots, pillow_color_pixel,
    pillow_color_table_parameters, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};
use crate::Image;
use crate::raster::{DynamicImage, RgbImage};

fn scalar_i_bytes(runs: &[(i32, usize)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for &(value, repetitions) in runs {
        for _ in 0..repetitions {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

#[test]
fn scalar_i_repeated_runs_match_scalar_inserts_and_preserve_slot_order() {
    let bytes = scalar_i_bytes(&[
        (7, 3_072),
        (2, 1_001),
        (7, 19),
        (0, 5),
        (255, 4_096),
        (-1, 3),
    ]);
    let maxcolors = 1_000_000;
    let (code_mask, code_polynomial) =
        pillow_color_table_parameters(maxcolors).expect("test limit has a Pillow table");
    let actual =
        getcolors_scalar_i_repeated_runs(&bytes, maxcolors as usize, code_mask, code_polynomial);
    let mut expected = PillowColorCounts::Direct(vec![0; code_mask as usize + 1]);
    let mut expected_colors = 0usize;
    for sample in bytes.chunks_exact(4) {
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        assert!(expected.count(
            &mut expected_colors,
            pixel,
            maxcolors as usize,
            code_mask,
            code_polynomial,
        ));
    }
    let expected = expected
        .into_slot_order(expected_colors)
        .into_iter()
        .map(|(_, (pixel, count))| {
            (
                count,
                super::super::FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes())),
            )
        })
        .collect::<Vec<_>>();
    let ScalarColorDomainCounts::Colors(actual) = actual else {
        panic!("repeated I input must produce colors");
    };
    assert_eq!(actual, expected);
}

#[test]
fn scalar_i_repeated_runs_preserve_maxcolors_limit() {
    let bytes = scalar_i_bytes(&[(1, 8), (2, 8), (3, 8)]);
    let (code_mask, code_polynomial) =
        pillow_color_table_parameters(2).expect("test limit has a Pillow table");
    assert!(matches!(
        getcolors_scalar_i_repeated_runs(&bytes, 2, code_mask, code_polynomial),
        ScalarColorDomainCounts::TooManyColors
    ));
}

#[test]
fn scalar_i_repeated_runs_yield_to_general_path_after_sparse_limit() {
    let mut runs = vec![(0, SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES)];
    runs.extend((1..=SCALAR_I_SPARSE_RUN_COLOR_LIMIT).map(|value| (value as i32, 1)));
    let bytes = scalar_i_bytes(&runs);
    let maxcolors = 100_000;
    let (code_mask, code_polynomial) =
        pillow_color_table_parameters(maxcolors as u32).expect("test limit has a Pillow table");

    assert!(matches!(
        getcolors_scalar_i_repeated_runs(&bytes, maxcolors, code_mask, code_polynomial,),
        ScalarColorDomainCounts::NotApplicable
    ));
}

#[test]
fn scalar_i_prefix_gate_rejects_short_run_before_high_cardinality_tail() {
    let mut runs = vec![(0, 64)];
    runs.extend((1..=192).map(|value| (value, 1)));
    let bytes = scalar_i_bytes(&runs);

    assert!(!scalar_i_prefix_has_long_run(&bytes));
    assert!(scalar_prefix_has_repeated_samples(&bytes));
}

#[test]
fn dense_table_admission_is_bounded_by_memory_and_scan_size() {
    assert_eq!(pillow_color_direct_table_slots(1023, 0), Some(1024));
    assert_eq!(pillow_color_direct_table_slots(2047, 1023), None);
    assert_eq!(pillow_color_direct_table_slots(2047, 1024), Some(2048));
    assert_eq!(
        pillow_color_direct_table_slots(1_048_575, 524_288),
        Some(PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES / 8)
    );
    assert_eq!(
        pillow_color_direct_table_slots(2_097_151, usize::MAX),
        None,
        "the dense table must stay below its byte cap even for large images"
    );
}

#[test]
fn dense_and_sparse_tables_keep_pillow_slot_order_and_limits() {
    let code_mask = 2047;
    let code_polynomial = 5;
    let pixels = [0, 2048, 1, 2049, 4096, 0, 2048, 8192];
    let mut dense = PillowColorCounts::new(code_mask, 0, 1024);
    assert!(matches!(&dense, PillowColorCounts::Direct(_)));
    let mut sparse = PillowColorCounts::Sparse(std::collections::HashMap::new());
    let mut dense_colors = 0;
    let mut sparse_colors = 0;

    for pixel in pixels {
        let dense_ok = dense.count(&mut dense_colors, pixel, 5, code_mask, code_polynomial);
        let sparse_ok = sparse.count(&mut sparse_colors, pixel, 5, code_mask, code_polynomial);
        assert_eq!(dense_ok, sparse_ok);
    }

    assert_eq!(dense_colors, 5);
    assert_eq!(dense_colors, sparse_colors);
    assert_eq!(
        dense.into_slot_order(dense_colors),
        sparse.into_slot_order(sparse_colors)
    );
    assert!(matches!(
        PillowColorCounts::new(code_mask, 0, 1023),
        PillowColorCounts::Sparse(_)
    ));
}

#[test]
fn large_rgb_grayscale_getcolors_matches_pillow_slot_order() {
    let mut raw = Vec::with_capacity(256 * 256 * 3);
    for _ in 0..256 {
        for gray in 0..=u8::MAX {
            raw.extend_from_slice(&[gray, gray, gray]);
        }
    }
    let rgb = RgbImage::from_raw(256, 256, raw)
        .expect("RGB test data must contain exactly three bytes per pixel");
    let image = Image::from_dynamic(DynamicImage::ImageRgb8(rgb.clone()), Some(crate::raster::Mode::ImageRgb8));
    let maxcolors = 1_000_000;
    let (code_mask, code_polynomial) =
        pillow_color_table_parameters(maxcolors).expect("test limit has a Pillow table");

    let mut reference = PillowColorCounts::Sparse(std::collections::HashMap::new());
    let mut reference_colors = 0;
    for pixel in rgb.pixels() {
        assert!(reference.count(
            &mut reference_colors,
            pillow_color_pixel(&pixel.0),
            maxcolors as usize,
            code_mask,
            code_polynomial,
        ));
    }
    let expected = reference
        .into_slot_order(reference_colors)
        .into_iter()
        .map(|(_, (pixel, count))| (count, pillow_color_components(pixel, 3)))
        .collect::<Vec<_>>();

    assert!(matches!(
        getcolors_rgb8_grayscale(&rgb, maxcolors as usize, code_mask, code_polynomial),
        RgbGrayscaleColorCounts::Colors(_)
    ));
    assert_eq!(
        image.getcolors(maxcolors).expect("count should succeed"),
        Some(expected)
    );
}

#[test]
fn large_rgb_grayscale_shortcut_rejects_mismatched_or_excess_colors() {
    let non_gray = RgbImage::from_raw(1, 1, vec![1, 2, 1])
        .expect("RGB test data must contain exactly three bytes per pixel");
    let (code_mask, code_polynomial) =
        pillow_color_table_parameters(1_000_000).expect("test limit has a Pillow table");
    assert!(matches!(
        getcolors_rgb8_grayscale(&non_gray, 1_000_000, code_mask, code_polynomial),
        RgbGrayscaleColorCounts::NotGrayscale
    ));

    let gray = RgbImage::from_raw(3, 1, vec![0, 0, 0, 127, 127, 127, 255, 255, 255])
        .expect("RGB test data must contain exactly three bytes per pixel");
    assert!(matches!(
        getcolors_rgb8_grayscale(&gray, 2, code_mask, code_polynomial),
        RgbGrayscaleColorCounts::TooManyColors
    ));
}
