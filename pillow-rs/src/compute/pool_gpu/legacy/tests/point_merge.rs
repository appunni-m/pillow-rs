use super::*;

#[test]
fn byte_point_fusion_requires_matching_native_mode() {
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![7]).unwrap());
    let gray_alpha = DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(1, 1, vec![7, 9]).unwrap());
    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![7, 9, 11]).unwrap());
    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![7, 9, 11, 13]).unwrap());

    assert!(gpu_byte_point_mode_allowed(&luma, None));
    assert!(gpu_byte_point_mode_allowed(&luma, Some("L")));
    assert!(!gpu_byte_point_mode_allowed(&luma, Some("P")));
    assert!(gpu_byte_point_mode_allowed(&gray_alpha, Some("LA")));
    assert!(!gpu_byte_point_mode_allowed(&gray_alpha, Some("L")));
    assert!(gpu_byte_point_mode_allowed(&rgb, Some("RGB")));
    assert!(gpu_byte_point_mode_allowed(&rgba, Some("RGBA")));
    assert!(!gpu_byte_point_mode_allowed(&rgba, Some("RGBX")));
}

#[test]
fn palette_first_rgb_merge_admission_is_narrow() {
    let mut palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("P band");
    palette
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("palette");
    let l0 = Image::frombytes("L", (2, 1), &[32, 33]).expect("G band");
    let l1 = Image::frombytes("L", (2, 1), &[48, 49]).expect("B band");
    let rgb = PipelineOp::Merge {
        mode: ColorMode::RGB,
        logical_mode: "RGB".to_owned(),
        bands: vec![palette.clone(), l0.clone(), l1.clone()].into(),
    };
    assert!(gpu_palette_first_rgb_merge_is_supported(&[rgb], Some("P")));

    let lab = PipelineOp::Merge {
        mode: ColorMode::RGB,
        logical_mode: "LAB".to_owned(),
        bands: vec![palette, l0, l1].into(),
    };
    assert!(!gpu_palette_first_rgb_merge_is_supported(&[lab], Some("P")));
}

#[test]
#[cfg(target_endian = "little")]
fn native_rgb_luma_merge_planner_bounds_compact_transfers_and_grid() {
    let odd = plan_gpu_native_rgb_merge(3, 2, 65_535, u32::MAX, u64::MAX)
        .expect("odd native L planes fit compact RGB transfers");
    assert_eq!(odd.input_bytes, 6);
    assert_eq!(odd.input_transfer_bytes, 8);
    assert_eq!(odd.auxiliary_bytes, 12);
    assert_eq!(odd.auxiliary_transfer_bytes, 12);
    assert_eq!(odd.output_bytes, 18);
    assert_eq!(odd.output_transfer_bytes, 20);
    assert_eq!(odd.output_word_count, 5);
    assert_eq!(odd.output_pixel_groups, 2);
    assert_eq!((odd.groups_x, odd.groups_y), (1, 1));

    let tiled = plan_gpu_native_rgb_merge(1_000, 1_000, 200, u32::MAX, u64::MAX)
        .expect("two-dimensional workgroup tiling fits per-axis limits");
    assert_eq!((tiled.groups_x, tiled.groups_y), (200, 5));
    assert!(tiled.groups_x <= 200 && tiled.groups_y <= 200);
    assert!(plan_gpu_native_rgb_merge(1_000, 1_000, 1, u32::MAX, u64::MAX).is_none());

    let full_4k = plan_gpu_native_rgb_merge(4_096, 4_096, 65_535, u32::MAX, u64::MAX)
        .expect("4096x4096 compact RGB output fits the default workgroup axis");
    assert_eq!(full_4k.input_bytes, 16_777_216);
    assert_eq!(full_4k.auxiliary_bytes, 33_554_432);
    assert_eq!(full_4k.output_bytes, 50_331_648);
    assert_eq!(full_4k.groups_y, 1);

    assert!(plan_gpu_native_rgb_merge(3, 2, 65_535, 19, u64::MAX).is_none());
    assert!(plan_gpu_native_rgb_merge(3, 2, 65_535, u32::MAX, 19).is_none());
    assert!(plan_gpu_native_rgb_merge(0, 1, 65_535, u32::MAX, u64::MAX).is_none());
    assert!(plan_gpu_native_rgb_merge(1, 1, 0, u32::MAX, u64::MAX).is_none());
}

#[test]
#[cfg(target_endian = "little")]
fn native_rgb_luma_merge_admission_requires_exact_l_planes() {
    let first = Image::frombytes("L", (2, 1), &[1, 2]).expect("R band");
    let green = Image::frombytes("L", (2, 1), &[3, 4]).expect("G band");
    let blue = Image::frombytes("L", (2, 1), &[5, 6]).expect("B band");
    let image =
        DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![1, 2]).expect("native L source"));
    let supported = PipelineOp::Merge {
        mode: ColorMode::RGB,
        logical_mode: "RGB".to_owned(),
        bands: vec![first.clone(), green.clone(), blue.clone()].into(),
    };
    assert!(
        gpu_native_rgb_luma_merge_bands(std::slice::from_ref(&supported), &image, Some("L"))
            .is_some()
    );
    assert!(
        gpu_native_rgb_luma_merge_bands(std::slice::from_ref(&supported), &image, Some("RGB"))
            .is_none()
    );

    let lab = PipelineOp::Merge {
        mode: ColorMode::RGB,
        logical_mode: "LAB".to_owned(),
        bands: vec![first, green, blue].into(),
    };
    assert!(
        gpu_native_rgb_luma_merge_bands(std::slice::from_ref(&lab), &image, Some("L")).is_none()
    );
}

#[test]
fn palette_first_rgb_merge_native_gpu_preserves_index_bytes() {
    let mut palette = Image::frombytes("P", (2, 1), &[1, 2]).expect("P band");
    palette
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("palette");
    let green = Image::frombytes("L", (2, 1), &[32, 33]).expect("G band");
    let blue = Image::frombytes("L", (2, 1), &[48, 49]).expect("B band");
    let merged = crate::image_merge("RGB", &[palette, green, blue]).expect("RGB merge");
    let expected = merged
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU merge");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match merged.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
        Err(error) => panic!("native GPU palette-first merge failed: {error}"),
    };
    assert_eq!(actual, [1, 32, 48, 2, 33, 49]);
    assert_eq!(actual, expected);
    let telemetry =
        Backend::take_pipeline_telemetry().expect("native GPU merge must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
#[cfg(target_endian = "little")]
fn native_rgb_luma_merge_gpu_preserves_odd_tail_and_compact_transfer() {
    let mapped_input = primary_readback_test_gpu().is_some();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (width, height) in [(1, 1), (3, 2), (17, 17)] {
        let pixels = width * height;
        let red = (0..pixels)
            .map(|index| (index * 31 + 1) as u8)
            .collect::<Vec<_>>();
        let green = (0..pixels)
            .map(|index| (index * 47 + 11) as u8)
            .collect::<Vec<_>>();
        let blue = (0..pixels)
            .map(|index| (index * 59 + 21) as u8)
            .collect::<Vec<_>>();
        let red_image = Image::frombytes("L", (width, height), &red).expect("R band");
        let green_image = Image::frombytes("L", (width, height), &green).expect("G band");
        let blue_image = Image::frombytes("L", (width, height), &blue).expect("B band");
        let merged =
            crate::image_merge("RGB", &[red_image, green_image, blue_image]).expect("RGB merge");
        let expected = merged
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU RGB merge");
        let Some(actual) = native_gpu_result_or_skip(
            merged.use_backend(Backend::Gpu).tobytes(),
            format_args!("native GPU RGB/L merge failed for {width}x{height}"),
        ) else {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        };
        assert_eq!(actual, expected, "native RGB/L merge {width}x{height}");
        if (width, height) == (3, 2) {
            assert_eq!(
                actual,
                [
                    1, 11, 21, 32, 58, 80, 63, 105, 139, 94, 152, 198, 125, 199, 1, 156, 246, 60,
                ]
            );
        }
        let receipt = Backend::take_pipeline_telemetry()
            .expect("native GPU RGB/L merge must publish a receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.expect("native GPU RGB/L merge resources");
        let input_transfer = u64::from(pixels.div_ceil(4) * 4);
        let auxiliary_transfer = u64::from((pixels * 2).div_ceil(4) * 4);
        let output_transfer = u64::from((pixels * 3).div_ceil(4) * 4);
        assert_eq!(resources.upload_bytes, input_transfer + auxiliary_transfer);
        assert_eq!(resources.auxiliary_bytes, auxiliary_transfer);
        assert_eq!(resources.readback_bytes, output_transfer);
        assert_eq!(
            resources.full_frame_copy_count,
            if mapped_input { 0 } else { 4 }
        );
        assert_eq!(resources.mode_conversion_count, 0);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn contrast_mean_after_exact_putpixel_prefix_matches_materialized_prefix() {
    let cases = [
        (
            DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![10, 20]).unwrap()),
            Some("L"),
            (1, 0, (80, 0, 0, 255)),
        ),
        (
            DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(2, 1, vec![10, 200, 20, 100]).unwrap(),
            ),
            Some("LA"),
            (1, 0, (80, 0, 0, 220)),
        ),
        (
            DynamicImage::ImageRgb8(
                RgbImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60]).unwrap(),
            ),
            Some("RGB"),
            (1, 0, (80, 90, 100, 255)),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80]).unwrap(),
            ),
            Some("RGBA"),
            (1, 0, (80, 90, 100, 220)),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![10, 20, 30, 40, 50, 60, 70, 80]).unwrap(),
            ),
            Some("CMYK"),
            (1, 0, (80, 90, 100, 220)),
        ),
    ];
    for (image, mode, (x, y, color)) in cases {
        assert_contrast_mean_after_exact_putpixel_case(image, mode, x, y, color);
    }

    let indexed = [
        PipelineOp::PutPixel {
            x: 0,
            y: 0,
            color: (1, 2, 3, 255),
            palette_index: true,
        },
        PipelineOp::Contrast { factor: 0.5 },
    ];
    let image = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![7]).unwrap());
    assert_eq!(
        gpu_contrast_mean_after_exact_prefix(&image, &indexed, Some("P")),
        None
    );
}

fn assert_contrast_mean_after_exact_putpixel_case(
    image: DynamicImage,
    mode: Option<&str>,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
) {
    let ops = [
        PipelineOp::PutPixel {
            x,
            y,
            color,
            palette_index: false,
        },
        PipelineOp::Contrast { factor: 0.5 },
    ];
    let prefixed = crate::compute::pool_cpu::ops::effects::op_put_pixel(&image, x, y, color)
        .expect("exact PutPixel prefix");
    assert_eq!(
        gpu_contrast_mean_after_exact_prefix(&image, &ops, mode),
        gpu_contrast_mean(&prefixed, mode),
        "mode {mode:?}"
    );
}
