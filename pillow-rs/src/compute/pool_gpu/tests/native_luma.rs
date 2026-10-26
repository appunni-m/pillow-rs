use super::*;

#[test]
fn extract_band_native_gpu_preserves_raw_color_mode_channels() {
    let cases = [
        ("L", 0, vec![71, 19], vec![71, 19]),
        ("LA", 1, vec![10, 201, 20, 202], vec![201, 202]),
        ("RGB", 0, vec![1, 2, 3, 11, 12, 13], vec![1, 11]),
        ("RGB", 1, vec![1, 2, 3, 11, 12, 13], vec![2, 12]),
        ("RGB", 2, vec![1, 2, 3, 11, 12, 13], vec![3, 13]),
        (
            "RGBA",
            0,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![21, 31],
        ),
        (
            "RGBA",
            1,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![22, 32],
        ),
        (
            "RGBA",
            2,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![23, 33],
        ),
        (
            "RGBA",
            3,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![24, 34],
        ),
        ("CMYK", 3, vec![1, 2, 3, 4, 11, 12, 13, 14], vec![4, 14]),
        ("HSV", 1, vec![21, 22, 23, 31, 32, 33], vec![22, 32]),
        ("YCbCr", 2, vec![41, 42, 43, 51, 52, 53], vec![43, 53]),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channel, source_bytes, expected_bytes) in cases {
        let source = Image::frombytes(mode, (2, 1), &source_bytes).expect("source image");
        let expected = source
            .getchannel(channel)
            .expect("channel operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU channel extraction");
        assert_eq!(expected, expected_bytes);
        let _ = crate::compute::take_pipeline_resource_telemetry();
        // `use_backend` only locks deferred work. The public getchannel
        // fast paths for single-band L and tiny RGB can execute eagerly
        // before a lock on the returned image is applied, so construct
        // the same ExtractBand pipeline directly for this GPU-kernel test.
        let actual = match Image::push_op(
            &source,
            PipelineOp::ExtractBand {
                index: u8::try_from(channel).expect("valid channel index"),
            },
        )
        .use_backend(Backend::Gpu)
        .tobytes()
        {
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
            Err(error) => panic!("native GPU {mode} channel extraction failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} channel extraction parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU channel extraction must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        let resources = telemetry
            .8
            .expect("native GPU channel extraction must publish resource telemetry");
        assert_eq!(
            resources.upload_bytes,
            source_bytes.len().div_ceil(4) as u64 * 4,
            "{mode} upload must keep its native byte layout"
        );
        assert_eq!(resources.readback_bytes, 4);
        assert_eq!(resources.mode_conversion_count, 0);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_expand_requires_matching_singleton_layout() {
    let expand = PipelineOp::Expand {
        border: 1,
        fill: (7, 11, 13, 17),
    };
    assert_gpu_native_expand_matching_layouts(&expand);
    assert_gpu_native_expand_rejects_mismatches(&expand);
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_expand_matching_layouts(expand: &PipelineOp) {
    let cases = [
        (
            DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![3, 5]).unwrap()),
            None,
            Some(1),
        ),
        (
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(2, 1, vec![3, 19, 5, 23]).unwrap()),
            None,
            Some(2),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("RGB"),
            Some(3),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("HSV"),
            Some(3),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17, 19, 23]).unwrap(),
            ),
            None,
            Some(4),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17, 19, 23]).unwrap(),
            ),
            Some("CMYK"),
            Some(4),
        ),
    ];
    for (image, mode, expected) in cases {
        assert_eq!(
            super::gpu_native_expand_channels(std::slice::from_ref(expand), &image, mode),
            expected,
            "mode {mode:?}"
        );
    }
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_expand_rejects_mismatches(expand: &PipelineOp) {
    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![1, 2, 3]).unwrap());
    assert_eq!(
        super::gpu_native_expand_channels(std::slice::from_ref(expand), &rgb, Some("CMYK")),
        None
    );
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![3]).unwrap());
    assert_eq!(
        super::gpu_native_expand_channels(std::slice::from_ref(expand), &luma, Some("1")),
        None
    );
    let multiple = [expand.clone(), PipelineOp::Duplicate];
    assert_eq!(
        super::gpu_native_expand_channels(&multiple, &rgb, Some("RGB")),
        None
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_grayscale_inputs_require_singleton_exact_storage() {
    let grayscale = PipelineOp::Grayscale;
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(3, 1, vec![12, 34, 56, 78, 90, 123, 234, 210, 98]).unwrap(),
    );
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            1,
            vec![12, 34, 56, 78, 90, 123, 234, 210, 98, 76, 54, 32],
        )
        .unwrap(),
    );
    assert_gpu_native_grayscale_rgb_input_modes(&grayscale, &rgb);
    assert_gpu_native_grayscale_rgba_input_modes(&grayscale, &rgba);
    assert_gpu_native_grayscale_rejected_inputs(&grayscale, &rgb, &rgba);
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rgb_input_modes(grayscale: &PipelineOp, rgb: &DynamicImage) {
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        None
    ));
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("RGB")
    ));
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("YCbCr")
    ));
    assert!(!super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("HSV")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rgba_input_modes(grayscale: &PipelineOp, rgba: &DynamicImage) {
    assert!(!super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("RGB")
    ));
    assert!(super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("CMYK")
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        None
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("RGB")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rejected_inputs(
    grayscale: &PipelineOp,
    rgb: &DynamicImage,
    rgba: &DynamicImage,
) {
    let multiple = [grayscale.clone(), PipelineOp::Duplicate];
    assert!(!super::gpu_native_grayscale_rgb_input(
        &multiple,
        rgb,
        Some("RGB")
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        &multiple,
        rgba,
        Some("CMYK")
    ));
    let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
    assert!(!super::gpu_native_grayscale_rgb_input(
        &[PipelineOp::Grayscale],
        &empty,
        Some("RGB")
    ));
    let empty_cmyk = DynamicImage::ImageRgba8(RgbaImage::new(0, 1));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        &[PipelineOp::Grayscale],
        &empty_cmyk,
        Some("CMYK")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_convert_requires_nonempty_native_l_to_rgba() {
    let convert_rgba = PipelineOp::Convert {
        mode: ColorMode::RGBA,
        matrix: None,
        dither: None,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert!(super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &luma,
        None
    ));
    assert!(super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &luma,
        Some("L")
    ));

    let rgb = PipelineOp::Convert {
        mode: ColorMode::RGB,
        matrix: None,
        dither: None,
    };
    assert!(!super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&rgb),
        &luma,
        Some("L")
    ));
    let multiple = [convert_rgba.clone(), PipelineOp::Duplicate];
    assert!(!super::gpu_packed_luma_convert_input(
        &multiple,
        &luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &empty,
        Some("L")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_transform_admits_only_nonempty_native_l() {
    let transform = PipelineOp::Transform {
        w: 2,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert!(super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        None
    ));
    assert!(super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_transform_input(
        &[transform.clone(), PipelineOp::Duplicate],
        &luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &empty,
        Some("L")
    ));
}

#[test]
fn gpu_packed_luma_point_admits_lut_and_lowered_l_autocontrast() {
    let eval = PipelineOp::Eval {
        lut: vec![0; 256].into(),
    };
    let equalize = PipelineOp::Equalize;
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert_gpu_packed_luma_point_admissions(&eval, &equalize, &luma);
    assert_gpu_packed_luma_point_rejections(&eval, &equalize, &luma);
}

fn assert_gpu_packed_luma_point_admissions(
    eval: &PipelineOp,
    equalize: &PipelineOp,
    luma: &DynamicImage,
) {
    assert!(super::gpu_packed_luma_point_input(
        true,
        false,
        std::slice::from_ref(eval),
        luma,
        None
    ));
    assert!(super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(equalize),
        luma,
        Some("L")
    ));
}

fn assert_gpu_packed_luma_point_rejections(
    eval: &PipelineOp,
    equalize: &PipelineOp,
    luma: &DynamicImage,
) {
    assert!(!super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(equalize),
        luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(eval),
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        &[eval.clone(), PipelineOp::Duplicate],
        luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        &empty,
        Some("L")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_order_statistic_admits_only_exact_native_l_singletons() {
    let median = PipelineOp::MedianFilter { size: 3 };
    let max_filter = PipelineOp::MaxFilter { size: 3 };
    let min_filter = PipelineOp::MinFilter { size: 3 };
    let rank = PipelineOp::RankFilter { size: 9, rank: 40 };
    let rank_one = PipelineOp::RankFilter { size: 3, rank: 1 };
    let luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(3, 2, vec![5, 40, 250, 100, 80, 10]).unwrap());
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&median),
        &luma,
        None
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&median),
        &luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&rank),
        &luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&rank_one),
        &luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&max_filter),
        &luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&min_filter),
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&median),
        &luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MedianFilter { size: 5 }],
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::RankFilter { size: 7, rank: 20 }],
        &luma,
        Some("L")
    ));
    for rank in [0, 2, 8] {
        assert!(!super::gpu_packed_luma_order_statistic_input(
            &[PipelineOp::RankFilter { size: 3, rank }],
            &luma,
            Some("L")
        ));
    }
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MaxFilter { size: 5 }],
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MinFilter { size: 5 }],
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[median.clone(), PipelineOp::Duplicate],
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&median),
        &DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap()),
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(&median),
        &DynamicImage::ImageLuma8(GrayImage::new(0, 1)),
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
struct NativeByteFilterCases {
    blur: PipelineOp,
    box_blur: PipelineOp,
    median: PipelineOp,
    max_filter: PipelineOp,
    min_filter: PipelineOp,
    rank_filter: PipelineOp,
    binomial3x3_kernel: [f32; 9],
    filter_3x3: PipelineOp,
    filter_5x5: PipelineOp,
    luma: DynamicImage,
    aligned_luma: DynamicImage,
    la: DynamicImage,
}

#[cfg(target_endian = "little")]
fn native_byte_filter_cases() -> NativeByteFilterCases {
    let blur = PipelineOp::GaussianBlur { sigma: 2.0 };
    let box_blur = PipelineOp::BoxBlur { radius: 1 };
    let median = PipelineOp::MedianFilter { size: 3 };
    let max_filter = PipelineOp::MaxFilter { size: 3 };
    let min_filter = PipelineOp::MinFilter { size: 3 };
    let rank_filter = PipelineOp::RankFilter { size: 3, rank: 1 };
    let binomial3x3_kernel = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    let filter_3x3 = PipelineOp::Filter3x3 {
        kernel: binomial3x3_kernel,
        scale: 16.0,
        offset: 0.0,
    };
    let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
    let kernel = std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
    let filter_5x5 = PipelineOp::Filter5x5 {
        kernel,
        scale: 256.0,
        offset: 0.0,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(65, 47, vec![37; 65 * 47]).unwrap());
    let aligned_luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(8, 6, vec![37; 8 * 6]).unwrap());
    let la =
        DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(65, 47, vec![37; 65 * 47 * 2]).unwrap());
    NativeByteFilterCases {
        blur,
        box_blur,
        median,
        max_filter,
        min_filter,
        rank_filter,
        binomial3x3_kernel,
        filter_3x3,
        filter_5x5,
        luma,
        aligned_luma,
        la,
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_native_byte_filter_admits_exact_native_l_and_la_singletons() {
    let cases = native_byte_filter_cases();
    assert_luma_native_byte_filter_admission(&cases);
    assert_la_native_byte_filter_admission(&cases);
    assert_la_native_byte_filter_rejections(&cases);
    assert_native_byte_filter_mode_and_shape_rejections(&cases);
}

#[cfg(target_endian = "little")]
fn assert_luma_native_byte_filter_admission(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: -2.0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: 0.0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter5x5 {
            kernel: [1.0; 25],
            scale: 0.0,
            offset: 0.0,
        }],
        &cases.aligned_luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_la_native_byte_filter_admission(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_3x3),
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_3x3),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter3x3 {
            kernel: [1.0; 9],
            scale: 16.0,
            offset: 0.0,
        }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter3x3 {
            kernel: cases.binomial3x3_kernel,
            scale: 16.0,
            offset: 1.0,
        }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter5x5 {
            kernel: [1.0; 25],
            scale: 0.0,
            offset: 0.0,
        }],
        &cases.la,
        Some("LA")
    ));
}

#[cfg(target_endian = "little")]
fn assert_la_native_byte_filter_rejections(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.max_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.min_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.rank_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MedianFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MaxFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MinFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::RankFilter { size: 5, rank: 1 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::RankFilter { size: 3, rank: 9 }],
        &cases.la,
        Some("LA")
    ));
}

#[cfg(target_endian = "little")]
fn assert_native_byte_filter_mode_and_shape_rejections(cases: &NativeByteFilterCases) {
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.la,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlur { radius: 0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlur { radius: 2 }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlurXY {
            radius_x: 1.0,
            radius_y: 1.0,
            passes: 1,
        }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[cases.blur.clone(), PipelineOp::Duplicate],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[cases.box_blur.clone(), PipelineOp::Duplicate],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        Some("RGB")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &DynamicImage::ImageRgb8(RgbImage::from_raw(65, 47, vec![0; 65 * 47 * 3]).unwrap()),
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: f32::NAN }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &DynamicImage::ImageLuma8(GrayImage::new(0, 1)),
        Some("L")
    ));
}
