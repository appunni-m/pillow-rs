//! Focused contract tests for native SIMD Paste and CompositeModule paths.

#[test]
fn hsv_masked_paste_uses_native_three_channel_simd_path() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage, RgbImage};
    use std::sync::Arc;

    let destination = DynamicImage::ImageRgb8(
        RgbImage::from_raw(1, 1, vec![10, 80, 190]).expect("HSV destination bytes"),
    );
    let source = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(1, 1, vec![200, 60, 10]).expect("HSV source bytes"),
        ),
        Some("HSV".to_owned()),
    ));
    let mask = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![127]).expect("L mask bytes")),
        Some("L".to_owned()),
    ));
    let op = PipelineOp::Paste {
        source,
        x: 0,
        y: 0,
        w: 1,
        h: 1,
        mask: Some(mask),
        mask_alpha: false,
    };

    let result = super::simd_paste(&destination, &op, Some("HSV"))
        .expect("HSV masked Paste must use the native SIMD byte path");
    assert_eq!(result.as_bytes(), [105, 70, 100]);
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[test]
fn simd_l_masked_paste_uses_neon_and_keeps_exact_rounding_and_tail() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage};
    use std::sync::Arc;

    let (width, height) = (257usize, 2usize);
    let destination_bytes = (0..width * height)
        .map(|index| (index * 37 + 13) as u8)
        .collect::<Vec<_>>();
    let source_bytes = (0..width * height)
        .map(|index| (index * 91 + 7) as u8)
        .collect::<Vec<_>>();
    let mask_bytes = (0..width * height)
        .map(|index| index as u8)
        .collect::<Vec<_>>();
    let destination = DynamicImage::ImageLuma8(
        GrayImage::from_raw(width as u32, height as u32, destination_bytes.clone())
            .expect("L destination dimensions must be valid"),
    );
    let source = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageLuma8(
            GrayImage::from_raw(width as u32, height as u32, source_bytes.clone())
                .expect("L source dimensions must be valid"),
        ),
        Some("L".to_owned()),
    ));
    let mask = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageLuma8(
            GrayImage::from_raw(width as u32, height as u32, mask_bytes.clone())
                .expect("L mask dimensions must be valid"),
        ),
        Some("L".to_owned()),
    ));
    let op = PipelineOp::Paste {
        source,
        x: 0,
        y: 0,
        w: width as i32,
        h: height as i32,
        mask: Some(mask),
        mask_alpha: false,
    };

    let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
    crate::compute::begin_pipeline_operation_telemetry("Paste");
    let output = super::simd_paste(&destination, &op, Some("L"))
        .expect("SIMD L masked Paste must use its native NEON path");
    crate::compute::finish_pipeline_operation_telemetry();
    let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
    crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

    let expected = source_bytes
        .iter()
        .copied()
        .zip(destination_bytes.iter().copied())
        .zip(mask_bytes.iter().copied())
        .map(|((source, destination), mask)| {
            ((u32::from(source) * u32::from(mask)
                + u32::from(destination) * (255 - u32::from(mask))
                + 127)
                / 255) as u8
        })
        .collect::<Vec<_>>();
    assert_eq!(output.as_bytes(), expected);

    let operation = telemetry
        .first()
        .expect("SIMD Paste must publish operation telemetry");
    assert_eq!(operation.path, "neon-blend");
    assert_eq!(operation.vector_block_count, 32);
    assert_eq!(operation.scalar_tail_count, 2);
}

#[test]
fn simd_rgba_l_masked_paste_uses_native_mask_expansion_and_exact_clipped_tail() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage, RgbaImage};
    use std::sync::Arc;

    let (source_width, source_height) = (27usize, 4usize);
    let (destination_width, destination_height) = (23usize, 3usize);
    let source_bytes = (0..source_width * source_height * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped source sample fits in a byte")
                .wrapping_mul(37)
                .wrapping_add(13)
        })
        .collect::<Vec<_>>();
    let destination_bytes = (0..destination_width * destination_height * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped destination sample fits in a byte")
                .wrapping_mul(71)
                .wrapping_add(29)
        })
        .collect::<Vec<_>>();
    let mask_pattern = [0u8, 1, 127, 128, 254, 255];
    let mask_bytes = (0..source_width * source_height)
        .map(|index| mask_pattern[index % mask_pattern.len()])
        .collect::<Vec<_>>();
    let destination = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            destination_width as u32,
            destination_height as u32,
            destination_bytes.clone(),
        )
        .expect("RGBA destination dimensions must be valid"),
    );
    let source = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                source_width as u32,
                source_height as u32,
                source_bytes.clone(),
            )
            .expect("RGBA source dimensions must be valid"),
        ),
        Some("RGBA".to_owned()),
    ));
    let mask = Arc::new(crate::Image::from_dynamic(
        DynamicImage::ImageLuma8(
            GrayImage::from_raw(
                source_width as u32,
                source_height as u32,
                mask_bytes.clone(),
            )
            .expect("L mask dimensions must be valid"),
        ),
        Some("L".to_owned()),
    ));
    let op = PipelineOp::Paste {
        source,
        x: -2,
        y: 1,
        w: source_width as i32,
        h: source_height as i32,
        mask: Some(mask),
        mask_alpha: false,
    };

    let previous = crate::compute::Backend::set_pipeline_telemetry_enabled(true);
    crate::compute::begin_pipeline_operation_telemetry("Paste");
    let output = super::simd_paste(&destination, &op, Some("RGBA"))
        .expect("SIMD RGBA/L Paste must use its native mask expansion path");
    crate::compute::finish_pipeline_operation_telemetry();
    let telemetry = crate::compute::Backend::take_pipeline_operation_telemetry();
    crate::compute::Backend::set_pipeline_telemetry_enabled(previous);

    let mut expected = destination_bytes;
    // x=-2 shifts source and L mask by two pixels; y=1 leaves only two
    // complete source rows visible in the destination.
    for destination_y in 1..destination_height {
        let source_y = destination_y - 1;
        for destination_x in 0..destination_width {
            let source_x = destination_x + 2;
            let mask_value = u32::from(mask_bytes[source_y * source_width + source_x]);
            for channel in 0..4 {
                let source_index = (source_y * source_width + source_x) * 4 + channel;
                let destination_index =
                    (destination_y * destination_width + destination_x) * 4 + channel;
                let source_value = u32::from(source_bytes[source_index]);
                let destination_value = u32::from(expected[destination_index]);
                expected[destination_index] =
                    ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                        / 255) as u8;
            }
        }
    }
    assert_eq!(
        output.as_bytes(),
        expected,
        "RGBA alpha is blended as stored data"
    );

    let operation = telemetry
        .first()
        .expect("SIMD Paste must publish operation telemetry");
    let expected_path = if cfg!(all(target_arch = "aarch64", target_feature = "neon")) {
        "neon-rgba-l-blend"
    } else {
        "rgba-l-mask-vector"
    };
    assert_eq!(operation.path, expected_path);
    assert_eq!(operation.vector_block_count, 14);
    assert_eq!(operation.scalar_tail_count, 8);
}

#[test]
fn native_pa_masked_paste_plan_uses_packed_pa_bytes_and_clipped_mask() {
    let source_bytes = vec![17, 201, 83, 149, 26, 222, 91, 163, 240, 7];
    let destination_bytes = vec![31, 42, 53, 64, 75, 86, 97, 108];
    let mask_bytes = vec![0, 1, 127, 128, 254];
    let source_image = crate::raster::DynamicImage::ImageLumaA8(
        crate::raster::GrayAlphaImage::from_raw(5, 1, source_bytes.clone())
            .expect("PA source shape"),
    );
    let destination = crate::raster::DynamicImage::ImageLumaA8(
        crate::raster::GrayAlphaImage::from_raw(4, 1, destination_bytes.clone())
            .expect("PA destination shape"),
    );
    let source = std::sync::Arc::new(crate::Image::from_dynamic(
        source_image,
        Some("PA".to_owned()),
    ));
    let mask = std::sync::Arc::new(crate::Image::from_dynamic(
        crate::raster::DynamicImage::ImageLuma8(
            crate::raster::GrayImage::from_raw(5, 1, mask_bytes.clone()).expect("L mask shape"),
        ),
        Some("L".to_owned()),
    ));

    let plan = super::native_paste_plan_for_image(
        &destination,
        &source,
        -1,
        0,
        5,
        1,
        Some(&mask),
        false,
        Some("PA"),
    )
    .expect("PA/L plan is admitted directly");
    assert_eq!(plan.layout.mode, "PA");
    assert_eq!(plan.layout.channels, 2);
    assert_eq!((plan.region.source_left, plan.region.width), (1, 4));

    let source_pixels = source.materialized_shared().expect("native PA bytes");
    let mask_pixels = mask.materialized_shared().expect("native L mask bytes");
    let mut actual = destination.as_bytes().to_vec();
    assert!(super::native_paste_apply(
        &mut actual,
        source_pixels.as_bytes(),
        Some(mask_pixels.as_bytes()),
        4,
        1,
        plan,
        true,
    ));

    let mut expected = destination_bytes;
    for destination_pixel in 0..4 {
        let source_pixel = destination_pixel + 1;
        let weight = u32::from(mask_bytes[source_pixel]);
        for channel in 0..2 {
            let byte = destination_pixel * 2 + channel;
            let source_value = u32::from(source_bytes[source_pixel * 2 + channel]);
            let destination_value = u32::from(expected[byte]);
            expected[byte] =
                ((source_value * weight + destination_value * (255 - weight) + 127) / 255) as u8;
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn native_l_masked_paste_rows_keep_positive_clipped_short_tail_exact() {
    let destination = vec![11, 22, 33, 44, 55, 66];
    let source = (101..116).collect::<Vec<u8>>();
    let mask = vec![
        0, 128, 255, 254, 1, 127, 128, 254, 255, 0, 1, 127, 128, 254, 255,
    ];
    let region = super::native_paste_region(3, 2, 5, 3, 1, 1)
        .expect("positive offset should leave a clipped two-pixel row");
    assert_eq!((region.width, region.height), (2, 1));
    let plan = super::NativePastePlan {
        layout: super::NativePasteLayout {
            channels: 1,
            mode: "L",
        },
        mask: Some(super::NativePasteMaskLayout {
            channels: 1,
            value_index: 0,
            premultiplied: false,
            binary: false,
        }),
        region,
        source_width: 5,
        source_height: 3,
        luma16_mask_byte_shift: 0,
    };

    let mut actual = destination.clone();
    assert!(super::native_paste_apply(
        &mut actual,
        &source,
        Some(&mask),
        3,
        2,
        plan,
        true,
    ));

    let mut expected = destination;
    for index in 0..2 {
        let source_value = u32::from(source[index]);
        let destination_value = u32::from(expected[4 + index]);
        let mask_value = u32::from(mask[index]);
        expected[4 + index] =
            ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                as u8;
    }
    assert_eq!(actual.as_slice(), expected.as_slice());
}

#[test]
fn native_la_masked_paste_row_duplicates_l_mask_per_pixel_across_vector_tails() {
    for width in 1..=256 {
        let source = (0..width * 2)
            .map(|index| (index * 37 + 11) as u8)
            .collect::<Vec<_>>();
        let destination = (0..width * 2)
            .map(|index| (index * 53 + 7) as u8)
            .collect::<Vec<_>>();
        let mask = (0..width).map(|index| index as u8).collect::<Vec<_>>();
        let mut expected = destination.clone();
        for byte_index in 0..expected.len() {
            let mask_value = u32::from(mask[byte_index / 2]);
            let source_value = u32::from(source[byte_index]);
            let destination_value = u32::from(destination[byte_index]);
            expected[byte_index] =
                ((source_value * mask_value + destination_value * (255 - mask_value) + 127) / 255)
                    as u8;
        }

        for allow_short_masked_tail in [false, true] {
            let mut actual = destination.clone();
            let used_neon = super::native_paste_la_masked_row(
                &source,
                &mut actual,
                &mask,
                allow_short_masked_tail,
            );
            assert_eq!(
                actual, expected,
                "width {width}, padded tail {allow_short_masked_tail}"
            );
            assert_eq!(
                used_neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")) && width >= 8,
                "width {width}, padded tail {allow_short_masked_tail} NEON route"
            );
        }
    }
}

#[test]
fn native_rgb_l_masked_paste_row_expands_mask_across_vector_tails() {
    let masks = [0u8, 1, 127, 128, 254, 255];
    for width in 1..=24 {
        let source = (0..width * 3)
            .map(|index| (index * 37 + 11) as u8)
            .collect::<Vec<_>>();
        let destination = (0..width * 3)
            .map(|index| (index * 53 + 7) as u8)
            .collect::<Vec<_>>();
        let mask = (0..width)
            .map(|index| masks[index % masks.len()])
            .collect::<Vec<_>>();
        let mut expected = destination.clone();
        for (pixel, mask_value) in mask.iter().copied().enumerate() {
            for channel in 0..3 {
                let byte = pixel * 3 + channel;
                let source_value = u32::from(source[byte]);
                let destination_value = u32::from(destination[byte]);
                let mask_value = u32::from(mask_value);
                expected[byte] =
                    ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                        / 255) as u8;
            }
        }

        for allow_short_masked_tail in [false, true] {
            let mut actual = destination.clone();
            super::native_paste_rgb_l_masked_row(
                &source,
                &mut actual,
                &mask,
                allow_short_masked_tail,
            );
            assert_eq!(
                actual, expected,
                "width {width}, padded tail {allow_short_masked_tail}"
            );
        }
    }
}

#[test]
fn native_cmyk_l_masked_paste_row_expands_mask_across_vector_tails() {
    let masks = [0u8, 1, 127, 128, 254, 255];
    for width in 1..=131 {
        let source = (0..width * 4)
            .map(|index| (index * 37 + 11) as u8)
            .collect::<Vec<_>>();
        let destination = (0..width * 4)
            .map(|index| (index * 53 + 7) as u8)
            .collect::<Vec<_>>();
        let mask = (0..width)
            .map(|index| masks[index % masks.len()])
            .collect::<Vec<_>>();
        let mut expected = destination.clone();
        for (pixel, mask_value) in mask.iter().copied().enumerate() {
            for channel in 0..4 {
                let byte = pixel * 4 + channel;
                let source_value = u32::from(source[byte]);
                let destination_value = u32::from(destination[byte]);
                let mask_value = u32::from(mask_value);
                expected[byte] =
                    ((source_value * mask_value + destination_value * (255 - mask_value) + 127)
                        / 255) as u8;
            }
        }

        for allow_short_masked_tail in [false, true] {
            let mut actual = destination.clone();
            let (_, _, used_neon) = super::native_paste_cmyk_l_masked_row(
                &source,
                &mut actual,
                &mask,
                allow_short_masked_tail,
            );
            assert_eq!(
                used_neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")) && width >= 16,
                "width {width}, neon dispatch"
            );
            assert_eq!(
                actual, expected,
                "width {width}, padded tail {allow_short_masked_tail}"
            );
        }
    }
}

#[test]
fn native_rgb_to_rgba_paste_returns_unchanged_destination_for_empty_or_disjoint_regions() {
    use crate::raster::{DynamicImage, RgbImage, RgbaImage};

    let destination_bytes = vec![17, 31, 47, 83, 19, 37, 53, 97];
    let destination = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, destination_bytes.clone())
            .expect("test destination shape must be valid"),
    );
    let cases = [
        (RgbImage::new(0, 1), 0, 0, 0, 1),
        (RgbImage::new(1, 0), 0, 0, 1, 0),
        (RgbImage::new(1, 1), i32::MAX, 0, 1, 1),
    ];
    for (source, x, y, width, height) in cases {
        let source = DynamicImage::ImageRgb8(source);
        let actual = super::simd_paste_rgb_to_rgba(&destination, &source, x, y, width, height)
            .expect("empty and disjoint SIMD paste must preserve the receiver");
        assert_eq!(actual.as_bytes(), destination_bytes);
    }

    let empty_destination = DynamicImage::ImageRgba8(RgbaImage::new(0, 1));
    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(1, 1, vec![101, 127, 149]).expect("test source shape must be valid"),
    );
    let actual = super::simd_paste_rgb_to_rgba(&empty_destination, &source, 0, 0, 1, 1)
        .expect("zero-width destination paste must preserve the receiver");
    assert!(actual.as_bytes().is_empty());
}
