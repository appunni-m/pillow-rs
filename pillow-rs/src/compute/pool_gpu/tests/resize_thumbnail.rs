use super::*;

#[test]
fn f_pad_f64_native_preserves_heterogeneous_words() {
    let values = [
        0.1f32, -0.3, 1.7, 2.9, 3.5, -4.25, 9.0, 0.0, 8.0, 1.0, 5.0, 7.0,
    ];
    let source_bytes = values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::new(4, 3, "F", (0, 0, 0, 0)).expect("initial F source");
    let source_dynamic = source.materialize().expect("materialize initial F source");
    let putdata = bytes::Bytes::from(source_bytes.clone());
    let filters = [
        (ResampleFilter::Bilinear, 2i64),
        (ResampleFilter::Bicubic, 3),
        (ResampleFilter::Lanczos, 1),
        (ResampleFilter::Hamming, 5),
        (ResampleFilter::Box, 4),
    ];
    for (filter, _) in filters {
        let pad = PipelineOp::Pad {
            w: 5,
            h: 5,
            filter,
            color: None,
            centering: (0.5, 0.5),
        };
        assert!(gpu_f_pad_f64_is_exact(
            std::slice::from_ref(&pad),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_pad_f64_is_exact(
            &[
                PipelineOp::PutData {
                    data: putdata.clone(),
                    mode: PixelMode::F,
                },
                pad,
            ],
            &source_dynamic,
            Some("F")
        ));
    }

    let mut expected_source = source.clone();
    expected_source
        .putdata(&source_bytes)
        .expect("CPU F PutData");
    let mut actual_source = source;
    actual_source.putdata(&source_bytes).expect("GPU F PutData");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (filter, code) in filters {
        let expected = crate::ops::imageops::pad_with_input(
            &expected_source,
            5,
            5,
            Some(ResampleInput::Code(code)),
            ImageOpsColor::Scalar(0),
            crate::ops::imageops::CenteringInput::Default,
        )
        .expect("CPU F Pad operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F Pad");
        let actual = match crate::ops::imageops::pad_with_input(
            &actual_source,
            5,
            5,
            Some(ResampleInput::Code(code)),
            ImageOpsColor::Scalar(0),
            crate::ops::imageops::CenteringInput::Default,
        )
        .expect("GPU F Pad operation")
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
            Err(error) => panic!("native GPU F Pad {filter:?} failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU F Pad {filter:?}");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU F Pad must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(4));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_thumbnail_constant_source_uses_exact_gpu_resize() {
    let source = Image::new(16, 16, "F", (0, 0, 0, 0)).expect("constant F thumbnail source");
    let mut expected_image = source.clone();
    expected_image
        .thumbnail((4, 4), None)
        .expect("CPU constant F thumbnail operation");
    let expected = expected_image
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU constant F thumbnail");

    let mut actual_image = source;
    actual_image
        .thumbnail((4, 4), None)
        .expect("GPU constant F thumbnail operation");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match actual_image.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU constant F thumbnail failed: {error}"),
    };
    assert_eq!(actual, expected, "constant F thumbnail parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("constant F thumbnail must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_thumbnail_constant_admission_rejects_reducing_overflow() {
    let words = 25usize.saturating_mul(38);
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            25,
            38,
            (0..words)
                .flat_map(|_| 0x7f27_69eeu32.to_le_bytes())
                .collect(),
        )
        .expect("constant finite F source"),
    );
    assert_eq!(
        gpu_f_source_constant_bits(&source, Some("F")),
        Some(0x7f27_69ee)
    );
    let thumbnail = PipelineOp::Thumbnail {
        w: 10,
        h: 7,
        filter: ResampleFilter::Bilinear,
    };
    assert!(!gpu_f_thumbnail_constant_is_exact(
        &thumbnail,
        (25, 38),
        &source,
        Some("F"),
    ));
}

#[test]
fn f_thumbnail_without_reducing_gap_uses_exact_f64_resize() {
    // Pillow skips the reducing-gap pass when both factors are one, then
    // runs the same filtered F resize used by Image.resize. The planner
    // previously kept every non-nearest F Thumbnail on host control even
    // in this no-reduce case, so marker 9 could not carry heterogeneous
    // non-dyadic words through the native path.
    let values = [0.1f32, -0.3, 1.7, 2.9];
    let source_bytes = values
        .iter()
        .cycle()
        .take(8 * 4)
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let source = Image::frombytes("F", (8, 4), &source_bytes).expect("F thumbnail source");
    let filters = ["BOX", "BILINEAR", "BICUBIC", "LANCZOS", "HAMMING"];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for filter in filters {
        let mut expected_image = source.clone();
        expected_image
            .thumbnail((6, 3), Some(ResampleInput::Name(filter.into())))
            .expect("CPU no-reduce F thumbnail");
        let expected = expected_image
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU no-reduce F thumbnail bytes");

        let mut actual_image = source.clone();
        actual_image
            .thumbnail((6, 3), Some(ResampleInput::Name(filter.into())))
            .expect("GPU no-reduce F thumbnail");
        let actual = match actual_image.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native no-reduce F thumbnail failed ({filter}): {error}"),
        };
        assert_eq!(actual, expected, "no-reduce F thumbnail parity ({filter})");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native no-reduce F thumbnail must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_filtered_chain_native_matches_cpu() {
    let values = [0.1f32, -0.3, 1.7, 2.9];
    let source_bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
    let expected = source
        .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
        .expect("CPU first chain resize")
        .resize((2, 2), Some(ResampleInput::Name("LANCZOS".into())), None)
        .expect("CPU second chain resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU filtered F chain");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
        .expect("GPU first chain resize")
        .resize((2, 2), Some(ResampleInput::Name("LANCZOS".into())), None)
        .expect("GPU second chain resize")
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
        Err(error) => panic!("native GPU filtered F chain failed: {error}"),
    };
    assert_eq!(actual, expected, "filtered F chain parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU filtered F chain must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.2, 2);
    assert_eq!(telemetry.6, Some(4));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_relocation_chain_native_matches_cpu() {
    fn source() -> Image {
        Image::frombytes(
            "F",
            (2, 2),
            &[0.1f32, -0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .expect("F relocation-chain source")
    }

    let cases = [
        ((3, 2), PipelineOp::Mirror, (2, 2), "mirror"),
        ((3, 2), PipelineOp::Flip, (2, 2), "flip"),
        (
            (3, 2),
            PipelineOp::Transpose {
                method: crate::pipeline::TransposeMethod::Rotate90,
            },
            (2, 2),
            "transpose",
        ),
        ((3, 2), PipelineOp::Offset { x: 1, y: -1 }, (2, 2), "offset"),
        ((3, 2), PipelineOp::Duplicate, (2, 2), "duplicate"),
        (
            (3, 2),
            PipelineOp::Crop {
                left: 0,
                top: 0,
                right: 3,
                bottom: 2,
            },
            (2, 2),
            "crop",
        ),
        (
            (4, 4),
            PipelineOp::CropBorder { border: 1 },
            (3, 3),
            "crop-border",
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (first_dimensions, relocation, final_dimensions, name) in cases {
        let first = source()
            .resize(
                (i64::from(first_dimensions.0), i64::from(first_dimensions.1)),
                Some(ResampleInput::Name("BICUBIC".into())),
                None,
            )
            .expect("CPU first relocation-chain resize");
        let expected = Image::push_op(&first, relocation.clone())
            .resize(
                (i64::from(final_dimensions.0), i64::from(final_dimensions.1)),
                Some(ResampleInput::Name("LANCZOS".into())),
                None,
            )
            .expect("CPU second relocation-chain resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("CPU {name} relocation chain failed: {error}"));

        let first = source()
            .resize(
                (i64::from(first_dimensions.0), i64::from(first_dimensions.1)),
                Some(ResampleInput::Name("BICUBIC".into())),
                None,
            )
            .expect("GPU first relocation-chain resize");
        let actual = match Image::push_op(&first, relocation)
            .resize(
                (i64::from(final_dimensions.0), i64::from(final_dimensions.1)),
                Some(ResampleInput::Name("LANCZOS".into())),
                None,
            )
            .expect("GPU second relocation-chain resize")
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
            Err(error) => panic!("native GPU {name} relocation chain failed: {error}"),
        };
        assert_eq!(
            actual, expected,
            "native GPU F {name} relocation chain parity"
        );
        let telemetry = Backend::take_pipeline_telemetry()
            .unwrap_or_else(|| panic!("native GPU F {name} chain missing telemetry"));
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.2, 3);
        assert_eq!(telemetry.6, Some(5));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_nearest_relocation_chain_native_matches_cpu() {
    fn source() -> Image {
        Image::frombytes(
            "F",
            (2, 2),
            &[0.1f32, -0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .expect("F nearest-chain source")
    }

    let pipeline = |image: Image| {
        image
            .resize((3, 3), Some(ResampleInput::Name("BICUBIC".into())), None)
            .expect("first nearest-chain resize")
            .resize((2, 2), Some(ResampleInput::Name("NEAREST".into())), None)
            .expect("nearest-chain copy resize")
            .resize((1, 3), Some(ResampleInput::Name("LANCZOS".into())), None)
            .expect("last nearest-chain resize")
    };
    let expected = pipeline(source())
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F nearest relocation chain");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match pipeline(source()).use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU F nearest relocation chain failed: {error}"),
    };
    assert_eq!(
        actual, expected,
        "native GPU F nearest relocation chain parity"
    );
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F nearest relocation chain must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.2, 3);
    assert_eq!(telemetry.6, Some(6));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_putdata_prefix_native_matches_cpu() {
    fn bytes(words: &[f32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // The deferred PutData is deliberately different from the initial
    // image. Pillow replaces the source words before evaluating the
    // non-dyadic resize; the GPU must prove and consume that same update
    // instead of applying the coefficients to stale upload bytes.
    let initial =
        Image::frombytes("F", (2, 2), &bytes(&[9.0, 8.0, 7.0, 6.0])).expect("initial F source");
    let replacement = bytes(&[0.1, -0.3, 1.7, 2.9]);

    let mut expected_image = initial.clone();
    expected_image.putdata(&replacement).expect("CPU F PutData");
    let expected = expected_image
        .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU F PutData resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F PutData resize bytes");

    let mut actual_image = initial;
    actual_image.putdata(&replacement).expect("GPU F PutData");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match actual_image
        .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("GPU F PutData resize")
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
        Err(error) => panic!("native GPU F PutData resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU F PutData resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F PutData resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.2, 2);
    assert_eq!(telemetry.6, Some(3));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_mixed_upscale_downscale_native_matches_cpu() {
    let values = [-9.7966165f32, 6.5041304];
    let source_bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::frombytes("F", (1, 2), &source_bytes).expect("F source");
    let expected = source
        .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("CPU mixed-geometry resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU mixed-geometry F resize");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("GPU mixed-geometry resize operation")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU mixed-geometry F resize");
    assert_eq!(actual, expected, "mixed F resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native mixed-geometry F resize must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_subnormal_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let source = Image::frombytes(
        "F",
        (2, 2),
        &bytes(&[0x0000_0001, 0x0000_0002, 0x0000_0003, 0x0000_0004]),
    )
    .expect("F subnormal source");
    let cases = [
        ((1i64, 2i64), ResampleInput::Name("BILINEAR".into())),
        ((1, 5), ResampleInput::Name("BICUBIC".into())),
        ((1, 5), ResampleInput::Name("LANCZOS".into())),
        ((1, 5), ResampleInput::Name("HAMMING".into())),
        ((1, 5), ResampleInput::Name("BOX".into())),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (size, filter) in cases {
        let expected = source
            .resize(size, Some(filter.clone()), None)
            .expect("CPU subnormal resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU subnormal F resize");
        let actual = match source
            .resize(size, Some(filter), None)
            .expect("GPU subnormal resize operation")
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
            Err(error) => panic!("native GPU subnormal F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "subnormal F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU subnormal F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
