use super::*;

#[test]
fn f_resize_dyadic_gpu_bytes_match_reference_and_publish_native_receipt() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let cases = [
        (
            (1, 2),
            (1, 7),
            ResampleInput::Name("NEAREST".into()),
            vec![1.0f32, 2.0],
            vec![
                1.0f32.to_bits(),
                1.0f32.to_bits(),
                1.0f32.to_bits(),
                1.0f32.to_bits(),
                2.0f32.to_bits(),
                2.0f32.to_bits(),
                2.0f32.to_bits(),
            ],
        ),
        (
            (1, 2),
            (1, 7),
            ResampleInput::Name("NEAREST".into()),
            vec![(-0.0f32), f32::NAN],
            vec![
                (-0.0f32).to_bits(),
                (-0.0f32).to_bits(),
                (-0.0f32).to_bits(),
                (-0.0f32).to_bits(),
                f32::NAN.to_bits(),
                f32::NAN.to_bits(),
                f32::NAN.to_bits(),
            ],
        ),
        (
            (2, 2),
            (4, 4),
            ResampleInput::Name("BILINEAR".into()),
            vec![1.0f32, 2.0, 4.0, 8.0],
            vec![
                0x3f80_0000,
                0x3fa0_0000,
                0x3fe0_0000,
                0x4000_0000,
                0x3fe0_0000,
                0x400c_0000,
                0x4044_0000,
                0x4060_0000,
                0x4050_0000,
                0x4082_0000,
                0x40b6_0000,
                0x40d0_0000,
                0x4080_0000,
                0x40a0_0000,
                0x40e0_0000,
                0x4100_0000,
            ],
        ),
        (
            (2, 2),
            (1, 2),
            ResampleInput::Name("BILINEAR".into()),
            vec![0.1f32, -0.3, 1.7, 2.9],
            vec![0xbdcc_ccce, 0x4013_3334],
        ),
        (
            (8, 1),
            (2, 1),
            ResampleInput::Name("BOX".into()),
            vec![
                2.0f32.powi(-80),
                2.0f32.powi(-79),
                2.0f32.powi(-78),
                2.0f32.powi(-77),
                2.0f32.powi(-76),
                2.0f32.powi(-75),
                2.0f32.powi(-74),
                2.0f32.powi(-73),
            ],
            vec![0x1870_0000, 0x1a70_0000],
        ),
        (
            (4, 2),
            (1, 1),
            ResampleInput::Name("BOX".into()),
            vec![1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0],
            vec![0x41ff_0000],
        ),
        (
            (4, 2),
            (1, 1),
            ResampleInput::Name("BOX".into()),
            vec![
                2.0f32.powi(-80),
                2.0f32.powi(-79),
                2.0f32.powi(-78),
                2.0f32.powi(-77),
                2.0f32.powi(-76),
                2.0f32.powi(-75),
                2.0f32.powi(-74),
                2.0f32.powi(-73),
            ]
            .into_iter()
            .map(|value| -value)
            .collect(),
            vec![0x99ff_0000],
        ),
        (
            (2, 2),
            (1, 1),
            ResampleInput::Name("BICUBIC".into()),
            vec![1.0f32, 2.0, 4.0, 8.0],
            vec![0x4070_0000],
        ),
        (
            (2, 2),
            (1, 1),
            ResampleInput::Name("LANCZOS".into()),
            vec![1.0f32, 2.0, 4.0, 8.0],
            vec![0x4070_0000],
        ),
        (
            (2, 2),
            (1, 1),
            ResampleInput::Name("HAMMING".into()),
            vec![1.0f32, 2.0, 4.0, 8.0],
            vec![0x4070_0000],
        ),
        (
            (4, 1),
            (3, 1),
            ResampleInput::Name("BOX".into()),
            vec![1.0f32, 2.0, 4.0, 8.0],
            vec![0x3f80_0000, 0x4040_0000, 0x4100_0000],
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (source_size, output_size, filter, values, expected_words) in cases {
        let source_bytes = bytes(
            &values
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
        );
        let source = Image::frombytes("F", source_size, &source_bytes).expect("F source");
        let actual = match source
            .resize(
                (i64::from(output_size.0), i64::from(output_size.1)),
                Some(filter),
                None,
            )
            .expect("resize operation")
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
            Err(error) => panic!("native GPU F resize failed: {error}"),
        };
        assert_eq!(actual, bytes(&expected_words));
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU resize must publish a telemetry receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }

    // Two sequential 2:1 Box reductions exercise the chain proof: the
    // first pass stores non-power-of-two f32 averages, and the second
    // pass must still match Pillow's f64 accumulation and f32 store.
    let chain_values = [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];
    let chain_source = Image::frombytes("F", (4, 2), &bytes(&chain_values.map(f32::to_bits)))
        .expect("F chain source");
    let chain_actual = match chain_source
        .resize((2, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("first Box resize")
        .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("second Box resize")
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
        Err(error) => panic!("native GPU F Box chain failed: {error}"),
    };
    assert_eq!(chain_actual, bytes(&[0x41ff_0000]));
    let chain_telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F Box chain must publish a telemetry receipt");
    assert_eq!(chain_telemetry.0, Some(Backend::Gpu));
    assert_eq!(chain_telemetry.1, Backend::Gpu);
    assert_eq!(chain_telemetry.6, Some(4));
    assert_eq!(chain_telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_box_copy_native_preserves_special_words() {
    let words: [u32; 7] = [
        0x7fc0_1234, // quiet NaN with a payload
        0xffc0_1234, // negative NaN with a payload
        0x7f80_0000, // positive infinity
        0xff80_0000, // negative infinity
        0x8000_0000, // negative zero
        0x0000_0000, // positive zero
        0x3fc0_0000, // finite control word
    ];
    let source_bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let source = Image::frombytes("F", (7, 1), &source_bytes).expect("F source");
    let expected_words: Vec<u32> = words
        .iter()
        .flat_map(|word| {
            let copied = if *word == 0x8000_0000 { 0 } else { *word };
            [copied, copied]
        })
        .chain(words.iter().flat_map(|word| {
            let copied = if *word == 0x8000_0000 { 0 } else { *word };
            [copied, copied]
        }))
        .collect();
    let expected: Vec<u8> = expected_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    let cpu = source
        .resize((14, 2), Some(ResampleInput::Name("BOX".into())), None)
        .expect("CPU resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU special-word resize");
    assert_eq!(cpu, expected, "Pillow Box copy special words");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((14, 2), Some(ResampleInput::Name("BOX".into())), None)
        .expect("GPU resize operation")
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
        Err(error) => panic!("native GPU special-word Box resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU Box copy special words");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU special-word Box resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);

    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_integer_emulation_matches_cpu_for_heterogeneous_significands() {
    let values = [0.1f32, 0.3, 1.7, 2.9];
    let source_bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
    let filters = ["BILINEAR", "BICUBIC"];
    let sizes = [(1i64, 2i64), (2, 1)];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for filter in filters {
        for size in sizes {
            let expected = source
                .resize(size, Some(ResampleInput::Name(filter.into())), None)
                .expect("CPU resize operation")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU F resize");
            let actual = match source
                .resize(size, Some(ResampleInput::Name(filter.into())), None)
                .expect("GPU resize operation")
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
                Err(error) => panic!("native GPU F {filter} resize failed: {error}"),
            };
            assert_eq!(actual, expected, "F {filter} integer emulation parity");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU F resize must publish a telemetry receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
    }

    // Exercise the signed accumulator and the second f32 boundary in a
    // single native two-axis Bilinear dispatch.
    let signed_values = [0.1f32, -0.3, 1.7, 2.9];
    let signed_bytes: Vec<u8> = signed_values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let signed_source = Image::frombytes("F", (2, 2), &signed_bytes).expect("F source");
    let expected = signed_source
        .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU two-axis resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU two-axis F resize");
    let actual = match signed_source
        .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("GPU two-axis resize operation")
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
        Err(error) => panic!("native GPU signed two-axis F resize failed: {error}"),
    };
    assert_eq!(actual, expected, "signed two-axis F resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU signed two-axis F resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_two_axis_native_non_dyadic_matches_cpu() {
    let values = [0.1f32, -0.3, 1.7, 2.9];
    let source_bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
    let expected = source
        .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F resize");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((1, 5), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("GPU resize operation")
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
        Err(error) => panic!("native GPU F two-axis resize failed: {error}"),
    };
    assert_eq!(actual, expected, "non-dyadic two-axis F resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F two-axis resize must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_native_mixed_upscale_downscale_matches_cpu() {
    let values = [1.0f32, 2.0, 4.0, 8.0];
    let source_bytes: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
    let expected = source
        .resize((4, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU mixed-geometry resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU mixed-geometry F resize");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((4, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("GPU mixed-geometry resize operation")
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
        Err(error) => panic!("native GPU mixed-geometry F resize failed: {error}"),
    };
    assert_eq!(actual, expected, "mixed-axis F resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU mixed-axis F resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);

    for filter in ["BICUBIC", "LANCZOS", "HAMMING"] {
        let expected = source
            .resize((4, 1), Some(ResampleInput::Name(filter.into())), None)
            .expect("CPU mixed-geometry filtered resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU mixed-geometry filtered F resize");
        let actual = source
            .resize((4, 1), Some(ResampleInput::Name(filter.into())), None)
            .expect("GPU mixed-geometry filtered resize operation")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native GPU mixed-geometry filtered F resize");
        assert_eq!(actual, expected, "mixed-axis {filter} F resize parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU mixed-axis filtered F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }

    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_native_preserves_proven_overflow_outputs() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let source_words = [0x7f7f_ffff, 0x7f7f_ffff, 0xff7f_ffff];
    let source = Image::frombytes("F", (3, 1), &bytes(&source_words)).expect("F source");
    let cases = [
        (
            ResampleInput::Name("BICUBIC".into()),
            [0x7f80_0000, 0xfe99_aea3],
        ),
        (
            ResampleInput::Name("LANCZOS".into()),
            [0x7f80_0000, 0xfe9d_8858],
        ),
        (
            ResampleInput::Name("HAMMING".into()),
            [0x7f7f_ffff, 0xfee4_133b],
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (filter, expected_words) in cases {
        let expected = source
            .resize((2, 1), Some(filter.clone()), None)
            .expect("CPU overflow resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU overflow F resize");
        assert_eq!(expected, bytes(&expected_words));

        let actual = match source
            .resize((2, 1), Some(filter), None)
            .expect("GPU overflow resize operation")
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
            Err(error) => panic!("native GPU overflow F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU overflow F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU overflow F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
