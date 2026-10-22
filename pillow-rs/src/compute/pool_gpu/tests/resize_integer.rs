use super::*;

#[test]
fn i_filtered_resize_native_gpu_preserves_int32_rounding() {
    let mut values = vec![0i32; 9 * 8];
    values[3 * 9 + 2] = 100_000;
    values[6 * 9 + 7] = -100_000;
    let source_bytes = values
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::frombytes("I", (9, 8), &source_bytes).expect("I source");
    let cases = [
        ((9, 3), ResampleInput::Name("BILINEAR".into())),
        ((9, 3), ResampleInput::Name("BICUBIC".into())),
        ((9, 3), ResampleInput::Name("LANCZOS".into())),
        ((9, 3), ResampleInput::Name("HAMMING".into())),
        ((9, 3), ResampleInput::Name("BOX".into())),
        ((3, 3), ResampleInput::Name("BICUBIC".into())),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (size, filter) in cases {
        let resized = source
            .resize(size, Some(filter), None)
            .expect("I filtered resize");
        let expected = resized
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU I filtered resize");
        let actual = match resized.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU I filtered resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU I filtered resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU I filtered resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_filtered_box_resize_native_gpu_handles_extreme_words() {
    let values = [
        i32::MIN,
        i32::MAX,
        0,
        -1,
        i32::MAX,
        i32::MIN,
        1,
        7,
        100_000,
        -100_000,
        2_000_000_000,
        -2_000_000_000,
        17,
        31,
        4096,
        -4096,
    ];
    let source_bytes = values
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::frombytes("I", (4, 4), &source_bytes).expect("I source");
    let resized = source
        .resize((2, 2), Some(ResampleInput::Name("BOX".into())), None)
        .expect("I Box resize");
    let expected = resized
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU I Box resize");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match resized.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU I Box resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU I Box resize parity");
    let telemetry =
        Backend::take_pipeline_telemetry().expect("native GPU I Box resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_resize_identity_native_gpu_preserves_signed_words() {
    let values = [
        i32::MIN,
        -12345,
        -1,
        0,
        1,
        12345,
        i32::MAX,
        -2_000_000_001,
        2_000_000_001,
    ];
    let source_bytes = (0..72usize)
        .flat_map(|index| values[index % values.len()].to_le_bytes())
        .collect::<Vec<_>>();
    let source = Image::frombytes("I", (9, 8), &source_bytes).expect("I source");
    let resized = source
        .resize((9, 8), Some(ResampleInput::Code(3)), None)
        .expect("I identity resize");
    let expected = resized
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU I identity resize");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match resized.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU I identity resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU I identity resize parity");
    assert_eq!(actual, source.tobytes().expect("I source bytes"));
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU I identity resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_resize_identity_proof_is_narrow() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 3, i32::MIN.to_le_bytes().repeat(12)).unwrap(),
    );
    let same_size = PipelineOp::Resize {
        w: 4,
        h: 3,
        filter: ResampleFilter::Lanczos,
    };
    assert!(gpu_i_resize_identity_is_exact(
        std::slice::from_ref(&same_size),
        &image,
        Some("I")
    ));
    assert!(!gpu_i_resize_identity_is_exact(
        std::slice::from_ref(&same_size),
        &image,
        Some("F")
    ));
    assert!(!gpu_i_resize_identity_is_exact(
        &[same_size.clone(), PipelineOp::Mirror],
        &image,
        Some("I")
    ));
    assert!(!gpu_i_resize_identity_is_exact(
        &[PipelineOp::Resize {
            w: 3,
            h: 3,
            filter: ResampleFilter::Bilinear,
        }],
        &image,
        Some("I")
    ));
}

#[test]
fn i_resize_aligned_reducer_matches_general_exact_reducer() {
    let source_dimensions = (9, 8);
    let values = [
        i32::MIN,
        i32::MAX,
        -100_000,
        -1,
        0,
        1,
        100_000,
        i32::MIN + 1,
        i32::MAX - 1,
    ];
    let samples = (0..source_dimensions.0 * source_dimensions.1)
        .map(|index| values[((index * 17 + 3) % values.len() as u32) as usize])
        .collect::<Vec<_>>();

    for filter in [
        ResampleFilter::Bilinear,
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Box,
    ] {
        let (kernel, support) = filter_from_resample(filter);
        for (horizontal, source_axis, output_axis, line_count) in [
            (true, source_dimensions.0, 5, source_dimensions.1),
            (false, source_dimensions.1, 4, source_dimensions.0),
        ] {
            let coeffs = precompute_coeffs_f64(output_axis, source_axis, kernel, support);
            let plan = gpu_i_resize_f64_pass_plan(&coeffs, source_axis as usize)
                .unwrap_or_else(|| panic!("{filter:?} coefficient plan"));
            assert!(
                plan.outputs.iter().any(|output| output.aligned),
                "{filter:?} produced no aligned coefficient rows"
            );
            for output_index in 0..plan.outputs.len() {
                for line in 0..line_count as usize {
                    let aligned = gpu_i_resize_f64_sample(
                        &samples,
                        source_dimensions,
                        &coeffs,
                        output_index,
                        &plan,
                        horizontal,
                        line,
                    );
                    let general = gpu_i_resize_f64_sample_slow(
                        &samples,
                        source_dimensions,
                        &coeffs,
                        output_index,
                        horizontal,
                        line,
                    );
                    assert_eq!(
                        aligned, general,
                        "{filter:?} output {output_index}, line {line}, horizontal={horizontal}"
                    );
                }
            }
        }
    }
}

#[test]
fn i_filtered_resize_f64_proof_matches_bounded_samples() {
    let mut bytes = vec![0u8; 9 * 8 * 4];
    let index = (3 * 9 + 2) * 4;
    bytes[index..index + 4].copy_from_slice(&100_000i32.to_le_bytes());
    let image = DynamicImage::ImageRgba8(RgbaImage::from_raw(9, 8, bytes).unwrap());
    let resize = PipelineOp::Resize {
        w: 9,
        h: 3,
        filter: ResampleFilter::Bilinear,
    };
    assert!(gpu_i_resize_f64_is_exact(
        std::slice::from_ref(&resize),
        &image,
        Some("I")
    ));
    assert!(!gpu_i_resize_f64_is_exact(
        std::slice::from_ref(&resize),
        &image,
        Some("F")
    ));
}

#[test]
fn indexed_nearest_rotate_native_gpu_preserves_raw_samples() {
    let cases = [
        (
            "P",
            (5, 4),
            vec![
                0, 17, 255, 42, 3, 99, 128, 7, 64, 201, 11, 240, 31, 88, 156, 4, 222, 13, 77, 190,
            ],
            Some((23, 0, 0, 255)),
        ),
        (
            "1",
            (8, 4),
            vec![0b1001_0110, 0b0110_1001, 0b1111_0000, 0b0000_1111],
            Some((255, 255, 255, 255)),
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, size, raw, fill) in cases {
        let source = Image::frombytes(mode, size, &raw)
            .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
        let rotated = source
            .rotate(13.0, true, fill)
            .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU indexed rotate must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
        assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
        assert_eq!(telemetry.7, None, "{mode} fallback reason");
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn indexed_rotate_forces_nearest_and_reports_transposed_size() {
    let cases = [
        ("P", (2, 1), vec![17, 203]),
        ("1", (8, 2), vec![0b1001_0110, 0b0110_1001]),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, size, raw) in cases {
        let source = Image::frombytes(mode, size, &raw)
            .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
        let rotated = source
            .rotate_with_input(
                90.0,
                RotateResampleInput::Name("BILINEAR".to_owned()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
        assert_eq!(
            rotated.size().unwrap(),
            (size.1, size.0),
            "{mode} lazy size"
        );
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU indexed right-angle rotate must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
        assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
        assert_eq!(telemetry.7, None, "{mode} fallback reason");
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn typed_scalar_right_angle_rotate_uses_native_transpose() {
    // A right-angle rotate is Pillow's complete-sample relocation fast
    // path even when a non-nearest filter token was supplied.  Exercise
    // every typed storage representation: I/F words must survive as-is,
    // and I;16 must preserve its declared byte order in the low word.
    let cases = [
        (
            "I",
            (3, 2),
            vec![
                0i32.to_le_bytes(),
                (-17i32).to_le_bytes(),
                65_535i32.to_le_bytes(),
                i32::MIN.to_le_bytes(),
                42i32.to_le_bytes(),
                i32::MAX.to_le_bytes(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        ),
        (
            "F",
            (3, 2),
            vec![
                0.25f32.to_le_bytes(),
                (-17.5f32).to_le_bytes(),
                f32::from_bits(0x7f7f_ffff).to_le_bytes(),
                (-0.0f32).to_le_bytes(),
                42.0f32.to_le_bytes(),
                f32::from_bits(0x8000_0001).to_le_bytes(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        ),
        (
            "I;16",
            (3, 2),
            [1u16, 258, 32_767, 65_535, 42, 54_321]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
        (
            "I;16B",
            (3, 2),
            [1u16, 258, 32_767, 65_535, 42, 54_321]
                .into_iter()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>(),
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, size, raw) in cases {
        let source = Image::frombytes(mode, size, &raw)
            .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
        let rotated = source
            .rotate_with_input(
                90.0,
                RotateResampleInput::Name("BICUBIC".to_owned()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .unwrap_or_else(|error| panic!("{mode} rotate: {error}"));
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("{mode} CPU rotate: {error}"));
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU {mode} rotate failed: {error}"),
        };
        assert_eq!(
            actual, expected,
            "native GPU {mode} right-angle rotate parity"
        );
        let telemetry = Backend::take_pipeline_telemetry()
            .unwrap_or_else(|| panic!("native GPU {mode} rotate missing telemetry"));
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
        assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
        assert_eq!(telemetry.7, None, "{mode} fallback reason");
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn cmyk_nearest_rotate_native_gpu_preserves_raw_samples() {
    let source = Image::frombytes(
        "CMYK",
        (3, 2),
        &[
            0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255, 13, 47, 89,
            131, 173, 215, 241, 7,
        ],
    )
    .expect("CMYK source");
    let cases = [
        source
            .rotate(1.0, true, None)
            .expect("CMYK fractional rotate"),
        source
            .rotate_with_input(
                90.0,
                RotateResampleInput::Name("BICUBIC".to_owned()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .expect("CMYK right-angle rotate"),
        source
            .rotate_with_input(
                180.0,
                RotateResampleInput::Name("BICUBIC".to_owned()),
                RotateExpandInput::Boolean(false),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::None,
            )
            .expect("CMYK 180-degree rotate"),
        source
            .rotate(90.0, true, None)
            .expect("CMYK 90-degree rotate"),
        source
            .rotate(270.0, true, None)
            .expect("CMYK 270-degree rotate"),
        source
            .rotate_with_input(
                17.5,
                RotateResampleInput::Name("NEAREST".to_owned()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Values(vec![1.25, 0.75]),
                RotatePointInput::Values(vec![0.25, -0.5]),
                ImageOpsColor::Components(vec![10, 20, 30, 40]),
            )
            .expect("CMYK custom nearest rotate"),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for rotated in cases {
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU CMYK rotate");
        let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU CMYK rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "CMYK rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU CMYK rotate must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn cmyk_filtered_rotate_uses_native_gpu_with_exact_parity() {
    let source = Image::frombytes(
        "CMYK",
        (3, 2),
        &[
            0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255, 13, 47, 89,
            131, 173, 215, 241, 7,
        ],
    )
    .expect("CMYK source");
    let rotated = source
        .rotate_with_input(
            17.5,
            RotateResampleInput::Name("BICUBIC".to_owned()),
            RotateExpandInput::Boolean(true),
            RotatePointInput::Default,
            RotatePointInput::Default,
            ImageOpsColor::None,
        )
        .expect("CMYK filtered rotate");
    let expected = rotated
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU CMYK filtered rotate");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match rotated.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("CMYK filtered rotate failed: {error}"),
    };
    assert_eq!(actual, expected, "CMYK filtered rotate parity");
    let telemetry =
        Backend::take_pipeline_telemetry().expect("CMYK filtered rotate must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_filter_nearest_resize_chain_guard_is_narrow() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, (-12345i32).to_le_bytes().repeat(16)).unwrap(),
    );
    let filter = PipelineOp::Filter3x3 {
        kernel: [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
        scale: 1.0,
        offset: 0.0,
    };
    assert!(gpu_int_filter_resize_chain_is_supported(
        &[
            filter.clone(),
            PipelineOp::Resize {
                w: 3,
                h: 2,
                filter: ResampleFilter::Nearest,
            },
        ],
        &image,
    ));
    assert!(!gpu_int_filter_resize_chain_is_supported(
        &[
            filter.clone(),
            PipelineOp::Resize {
                w: 3,
                h: 2,
                filter: ResampleFilter::Bilinear,
            },
        ],
        &image,
    ));
    assert!(!gpu_int_filter_resize_chain_is_supported(
        &[
            PipelineOp::Resize {
                w: 3,
                h: 2,
                filter: ResampleFilter::Nearest,
            },
            filter,
        ],
        &image,
    ));
    assert!(!gpu_int_filter_resize_chain_is_supported(
        &[
            PipelineOp::Filter5x5 {
                kernel: [0.0; 25],
                scale: 1.0,
                offset: 0.0,
            },
            PipelineOp::Resize {
                w: 3,
                h: 2,
                filter: ResampleFilter::Nearest,
            },
        ],
        &image,
    ));
}
