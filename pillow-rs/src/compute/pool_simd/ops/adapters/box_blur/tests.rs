//! Focused contract tests for the SIMD BoxBlur family.

#[test]
fn rgb_box_blur_radius_one_direct_passes_match_cpu_at_edges_and_tails() {
    use crate::compute::pool_cpu::ops::filter::execute_box_blur;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, RgbImage};

    let operation = PipelineOp::BoxBlur { radius: 1 };
    for (width, height) in [
        (1u32, 1u32),
        (2, 3),
        (3, 2),
        (4, 5),
        (5, 9),
        (8, 3),
        (9, 4),
        (19, 7),
        (65, 47),
        (67, 53),
        (1024, 3),
    ] {
        let source: Vec<u8> = (0..width as usize * height as usize * 3)
            .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("RGB blur source shape"),
        );
        let expected = execute_box_blur(&image, 1).expect("CPU RGB BoxBlur");
        let actual =
            super::simd_box_blur(&image, &operation, Some("RGB")).expect("SIMD RGB BoxBlur");
        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "size={width}x{height}"
        );
    }
}

#[test]
fn luma_box_blur_radius_one_matches_cpu_at_edges_and_vector_tails() {
    use crate::compute::pool_cpu::ops::filter::execute_box_blur;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage};

    let operation = PipelineOp::BoxBlur { radius: 1 };
    for (width, height) in [
        (1u32, 1u32),
        (2, 3),
        (3, 2),
        (7, 5),
        (15, 2),
        (16, 3),
        (17, 5),
        (65, 47),
        (67, 53),
        (1024, 3),
    ] {
        let source: Vec<u8> = (0..width as usize * height as usize)
            .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, source).expect("L blur source shape"),
        );
        let expected = execute_box_blur(&image, 1).expect("CPU L BoxBlur");
        let actual = super::simd_box_blur(&image, &operation, Some("L")).expect("SIMD L BoxBlur");
        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "size={width}x{height}"
        );
    }
}

#[test]
fn luma_radius_one_reciprocal_matches_pillow_fixed_point_for_all_byte_sums() {
    let whole_weight = super::SIMD_BOX_BLUR_SCALE / 3;
    for sum in 0..=3 * u32::from(u8::MAX) {
        let pillow = sum
            .wrapping_mul(whole_weight)
            .wrapping_add(super::super::SIMD_BOX_BLUR_BIAS)
            >> 24;
        let reciprocal = ((sum + 1) * 21_846) >> 16;
        assert_eq!(reciprocal, pillow, "byte sum={sum}");
    }
}

#[test]
fn rgba_blur_vector_pack_preserves_byte_lanes() {
    let values = super::super::u32x16::new([
        0, 255, 1, 254, 127, 128, 17, 239, 33, 222, 64, 191, 85, 170, 100, 200,
    ]);
    assert_eq!(
        super::super::simd_blur_pack_u32x16_to_bytes(values),
        [
            0, 255, 1, 254, 127, 128, 17, 239, 33, 222, 64, 191, 85, 170, 100, 200
        ]
    );
}

#[test]
fn rgba_radius_one_direct_passes_match_generic_simd() {
    for radius in [1.0f32, 1.25, 1.9] {
        let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

        for (width, height) in [
            (1, 1),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (8, 3),
            (9, 4),
            (19, 7),
            (256, 3),
        ] {
            let dimensions = crate::checked_dims::CheckedDims::new(width, height, 4)
                .expect("test RGBA dimensions fit");
            let source: Vec<u8> = (0..dimensions.total_bytes())
                .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                .collect();

            let mut horizontal_expected = dimensions.alloc_buffer();
            super::simd_blur_rows::<false>(
                &source,
                &mut horizontal_expected,
                width as usize,
                height as usize,
                4,
                1,
                whole_weight,
                fractional_weight,
            );
            let mut horizontal_actual = dimensions.alloc_buffer();
            super::simd_rgba_horizontal_radius_one_rows(
                &source,
                &mut horizontal_actual,
                width as usize,
                height as usize,
                whole_weight,
                fractional_weight,
            );
            assert_eq!(
                horizontal_actual, horizontal_expected,
                "horizontal dimensions={width}x{height} radius={radius}"
            );

            let mut transposed = dimensions.alloc_buffer();
            super::simd_transpose_interleaved_rows(
                &source,
                &mut transposed,
                width as usize,
                height as usize,
                4,
            );
            let mut transposed_blurred = dimensions.alloc_buffer();
            super::simd_blur_rows::<false>(
                &transposed,
                &mut transposed_blurred,
                height as usize,
                width as usize,
                4,
                1,
                whole_weight,
                fractional_weight,
            );
            let mut vertical_expected = dimensions.alloc_buffer();
            super::simd_transpose_interleaved_rows(
                &transposed_blurred,
                &mut vertical_expected,
                height as usize,
                width as usize,
                4,
            );

            let mut vertical_actual = dimensions.alloc_buffer();
            super::simd_rgba_vertical_radius_one_rows(
                &source,
                &mut vertical_actual,
                width as usize,
                height as usize,
                whole_weight,
                fractional_weight,
            );
            assert_eq!(
                vertical_actual, vertical_expected,
                "vertical dimensions={width}x{height} radius={radius}"
            );
        }
    }
}

#[test]
fn rgb_radius_one_direct_vertical_passes_match_transposed_simd() {
    for radius in [1.0f32, 1.25, 1.9] {
        let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

        for (width, height) in [
            (1, 1),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (8, 3),
            (9, 4),
            (19, 7),
            (65, 47),
            (67, 53),
        ] {
            let dimensions = crate::checked_dims::CheckedDims::new(width, height, 3)
                .expect("test RGB dimensions fit");
            let source: Vec<u8> = (0..dimensions.total_bytes())
                .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                .collect();

            let mut expected = dimensions.alloc_buffer();
            super::simd_transpose_interleaved_rows(
                &source,
                &mut expected,
                width as usize,
                height as usize,
                3,
            );
            let mut expected_scratch = dimensions.alloc_buffer();
            for _ in 0..3 {
                super::simd_blur_rows::<false>(
                    &expected,
                    &mut expected_scratch,
                    height as usize,
                    width as usize,
                    3,
                    1,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut expected, &mut expected_scratch);
            }
            super::simd_transpose_interleaved_rows(
                &expected,
                &mut expected_scratch,
                height as usize,
                width as usize,
                3,
            );

            let mut actual = source;
            let mut actual_scratch = dimensions.alloc_buffer();
            for _ in 0..3 {
                super::simd_rgb_vertical_radius_one_rows(
                    &actual,
                    &mut actual_scratch,
                    width as usize,
                    height as usize,
                    whole_weight,
                    fractional_weight,
                );
                std::mem::swap(&mut actual, &mut actual_scratch);
            }
            assert_eq!(
                actual, expected_scratch,
                "vertical dimensions={width}x{height} radius={radius}"
            );
        }
    }
}

#[test]
fn rgb_radius_one_direct_horizontal_passes_match_generic_simd() {
    for radius in [1.0f32, 1.25, 1.9] {
        let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

        for (width, height) in [
            (1, 1),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (8, 3),
            (9, 4),
            (19, 7),
            (65, 47),
            (67, 53),
            (1024, 3),
        ] {
            let dimensions = crate::checked_dims::CheckedDims::new(width, height, 3)
                .expect("test RGB dimensions fit");
            let source: Vec<u8> = (0..dimensions.total_bytes())
                .map(|index| (index.wrapping_mul(73).wrapping_add(29) % 256) as u8)
                .collect();
            let mut expected = dimensions.alloc_buffer();
            super::simd_blur_rows::<false>(
                &source,
                &mut expected,
                width as usize,
                height as usize,
                3,
                1,
                whole_weight,
                fractional_weight,
            );
            let mut actual = dimensions.alloc_buffer();
            super::simd_rgb_horizontal_radius_one_rows(
                &source,
                &mut actual,
                width as usize,
                height as usize,
                whole_weight,
                fractional_weight,
            );
            assert_eq!(
                actual, expected,
                "horizontal dimensions={width}x{height} radius={radius}"
            );
        }
    }
}

fn assert_la_horizontal_radius_one_matches_generic(
    dimensions: &crate::checked_dims::CheckedDims,
    source: &[u8],
    whole_weight: u32,
    fractional_weight: u32,
    radius: f32,
) {
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    let mut expected = dimensions.alloc_buffer();
    super::simd_blur_rows::<false>(
        source,
        &mut expected,
        width,
        height,
        2,
        1,
        whole_weight,
        fractional_weight,
    );
    let transposed_dimensions =
        crate::checked_dims::CheckedDims::new(dimensions.height, dimensions.width, 2)
            .expect("transposed LA dimensions fit");
    let mut transposed = transposed_dimensions.alloc_buffer();
    super::simd_transpose_interleaved_rows(source, &mut transposed, width, height, 2);
    let mut transposed_actual = transposed_dimensions.alloc_buffer();
    super::simd_la_vertical_radius_one_rows(
        &transposed,
        &mut transposed_actual,
        height,
        width,
        whole_weight,
        fractional_weight,
    );
    let mut actual = dimensions.alloc_buffer();
    super::simd_transpose_interleaved_rows(&transposed_actual, &mut actual, height, width, 2);
    assert_eq!(
        actual, expected,
        "horizontal dimensions={width}x{height} radius={radius}"
    );
}

fn assert_la_vertical_radius_one_matches_generic(
    dimensions: &crate::checked_dims::CheckedDims,
    source: &[u8],
    whole_weight: u32,
    fractional_weight: u32,
    radius: f32,
) {
    let (width, height) = (dimensions.width as usize, dimensions.height as usize);
    let mut transposed = dimensions.alloc_buffer();
    super::simd_transpose_interleaved_rows(source, &mut transposed, width, height, 2);
    let mut transposed_blurred = dimensions.alloc_buffer();
    super::simd_blur_rows::<false>(
        &transposed,
        &mut transposed_blurred,
        height,
        width,
        2,
        1,
        whole_weight,
        fractional_weight,
    );
    let mut expected = dimensions.alloc_buffer();
    super::simd_transpose_interleaved_rows(&transposed_blurred, &mut expected, height, width, 2);
    let mut actual = dimensions.alloc_buffer();
    super::simd_la_vertical_radius_one_rows(
        source,
        &mut actual,
        width,
        height,
        whole_weight,
        fractional_weight,
    );
    assert_eq!(
        actual, expected,
        "vertical dimensions={width}x{height} radius={radius}"
    );
}

#[test]
fn la_radius_one_direct_passes_match_generic_simd() {
    for radius in [1.0f32, 1.25, 1.9] {
        let whole_weight = (super::SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight = super::SIMD_BOX_BLUR_SCALE.wrapping_sub(3 * whole_weight) / 2;

        for (width, height) in [
            (1, 1),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (8, 3),
            (9, 4),
            (19, 7),
            (65, 47),
        ] {
            let dimensions = crate::checked_dims::CheckedDims::new(width, height, 2)
                .expect("test LA dimensions fit");
            let source: Vec<u8> = (0..dimensions.total_bytes())
                .map(|index| (index.wrapping_mul(59).wrapping_add(17) % 256) as u8)
                .collect();
            assert_la_horizontal_radius_one_matches_generic(
                &dimensions,
                &source,
                whole_weight,
                fractional_weight,
                radius,
            );
            assert_la_vertical_radius_one_matches_generic(
                &dimensions,
                &source,
                whole_weight,
                fractional_weight,
                radius,
            );
        }
    }
}
