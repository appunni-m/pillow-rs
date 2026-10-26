use super::*;

#[test]
fn f_resize_ordered_shader_admission_checks_geometry_and_coefficients() {
    let source = Image::frombytes("F", (32, 24), &vec![0; 32 * 24 * 4]).expect("F source");
    let source_dynamic = source.materialize().expect("materialize F source");
    let op = PipelineOp::Resize {
        w: 16,
        h: 12,
        filter: ResampleFilter::Bicubic,
    };
    assert!(gpu_f_resize_f64_ordered_shader_is_supported(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_f64_ordered_shader_is_supported(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("I")
    ));

    let nearest = PipelineOp::Resize {
        w: 16,
        h: 12,
        filter: ResampleFilter::Nearest,
    };
    assert!(!gpu_f_resize_f64_ordered_shader_is_supported(
        std::slice::from_ref(&nearest),
        &source_dynamic,
        Some("F")
    ));

    let wide_row_source =
        Image::frombytes("F", (128, 8), &vec![0; 128 * 8 * 4]).expect("wide F source");
    let wide_row_dynamic = wide_row_source
        .materialize()
        .expect("materialize wide F source");
    let wide_row = PipelineOp::Resize {
        w: 4,
        h: 8,
        filter: ResampleFilter::Lanczos,
    };
    assert!(!gpu_f_resize_f64_ordered_shader_is_supported(
        std::slice::from_ref(&wide_row),
        &wide_row_dynamic,
        Some("F")
    ));
}

#[test]
fn f_resize_ordered_f64_two_tap_native_matches_cpu() {
    // The marker-9 exact-real reducer rejects this row because the tiny
    // second product is below the first product's f64 ulp.  Pillow rounds
    // the accumulator after the first FMA, then performs the second FMA;
    // marker 12 reproduces that ordered boundary without relaxed device
    // floats, including wider rows up to the bounded tap cap.
    let source_bytes = [
        44, 149, 220, 51, // 1.0271683e-7
        155, 14, 222, 56, // 1.0588505e-4
        62, 115, 42, 56, // 4.0638486e-5
    ];
    let source = Image::frombytes("F", (3, 1), &source_bytes).expect("F source");
    let source_dynamic = source.materialize().expect("materialize F source");
    let op = PipelineOp::Resize {
        w: 2,
        h: 1,
        filter: ResampleFilter::Bilinear,
    };
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    let expected = source
        .resize((2, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU ordered-f64 resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F resize bytes");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((2, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("GPU ordered-f64 resize")
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
        Err(error) => panic!("native GPU ordered-f64 resize failed: {error}"),
    };
    assert_eq!(actual, expected, "ordered f64 two-tap F resize");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("ordered-f64 F resize must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_ordered_f64_wider_tap_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // A three-tap Lanczos upscale is the first wider row that exposed the
    // old marker-12 count guard. Pillow still rounds each ordered f64 FMA
    // before the final f32 store; the bounded integer reducer now follows
    // that sequence for this finite heterogeneous row.
    let source = Image::frombytes(
        "F",
        (3, 1),
        &bytes(&[0x3fc3_2777, 0xbeb0_5e6a, 0x44db_fac1]),
    )
    .expect("F source");
    let source_dynamic = source.materialize().expect("materialize F source");
    let op = PipelineOp::Resize {
        w: 5,
        h: 1,
        filter: ResampleFilter::Lanczos,
    };
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    let filter = ResampleInput::Name("LANCZOS".into());
    let expected = source
        .resize((5, 1), Some(filter.clone()), None)
        .expect("CPU wider ordered-f64 resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F resize bytes");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((5, 1), Some(filter), None)
        .expect("GPU wider ordered-f64 resize")
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
        Err(error) => panic!("native GPU wider ordered-f64 resize failed: {error}"),
    };
    assert_eq!(actual, expected, "wider ordered f64 F resize");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("wider ordered-f64 F resize must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_arm64_wide_horizontal_vector_native_matches_pillow() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Pillow 12.2.0's arm64 FLOAT32 horizontal kernel changes from the
    // scalar FMA loop to separate product/ordered-add accumulation after
    // 15 taps. These rows were the first deterministic divergences when
    // the host and shader admitted every finite row through the FMA model:
    // Pillow produced c8be3d3d and bbc8afba, while that model produced
    // c9be3d3d and b9afc8bb respectively.
    let cases = [(16usize, 91u32, 0x3d3d_bec8), (32, 275, 0xbbc8_afba)];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (width, trial, expected_word) in cases {
        let mut seed = trial.wrapping_mul(0x9e37_79b9).wrapping_add(width as u32);
        let words: Vec<u32> = (0..width)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (125 << 23) | (seed & 0x8000_0000) | (seed & 0x007f_ffff)
            })
            .collect();
        let source_bytes = bytes(&words);
        let source =
            Image::frombytes("F", (width as u32, 1), &source_bytes).expect("wide F source");
        let source_dynamic = source.materialize().expect("materialize wide F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        };
        assert!(!gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let expected = bytes(&[expected_word]);
        assert_wide_f_cpu_reference(&source, &expected, width);

        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
            .expect("GPU wide F resize operation")
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
            Err(error) => panic!("native GPU wide F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "GPU wide F resize at width {width}");
        assert_wide_f_resize_telemetry();
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn assert_wide_f_resize_telemetry() {
    let telemetry =
        Backend::take_pipeline_telemetry().expect("wide F resize must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
}

fn assert_wide_f_cpu_reference(source: &Image, expected: &[u8], width: usize) {
    let cpu = source
        .resize((1, 1), Some(ResampleInput::Name("BILINEAR".into())), None)
        .expect("CPU wide F resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU wide F resize");
    assert_eq!(cpu, expected, "CPU wide F resize at width {width}");
}

#[test]
fn f_resize_wide_box_extreme_exponent_is_native() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // A 16-tap Box row has a max-normal word followed by ordinary values.
    // Its exact average is 0x7d7fffff; any proof that drops the high
    // exponent term can incorrectly produce the small-value average.
    let source = Image::frombytes(
        "F",
        (16, 1),
        &bytes(
            &[0x7f7f_ffff]
                .into_iter()
                .chain([0x3f80_0000; 15])
                .collect::<Vec<_>>(),
        ),
    )
    .expect("wide Box F source");
    let source_dynamic = source.materialize().expect("materialize wide Box F source");
    let op = PipelineOp::Resize {
        w: 1,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert!(!gpu_f_resize_integer_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));

    let expected = bytes(&[0x7d7f_ffff]);
    let cpu = source
        .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("CPU wide Box resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU wide Box resize");
    assert_eq!(cpu, expected);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((1, 1), Some(ResampleInput::Name("BOX".into())), None)
        .expect("GPU wide Box resize operation")
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
        Err(error) => panic!("native GPU wide Box resize failed: {error}"),
    };
    assert_eq!(actual, expected);
    let telemetry =
        Backend::take_pipeline_telemetry().expect("wide Box native resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_wide_lanczos_cancellation_native_matches_pillow() {
    fn bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    // This 48-tap Lanczos row was the first proven failure above the old
    // marker-12 cap: marker 9's exact-real GPU coefficients cancelled the
    // middle result to +0.0, while Pillow's src/libImaging/Resample.c
    // arm64 ordered reduction stores -2.0. The ordered reducer now models
    // that wider row after using the native multiply/divide order for the
    // Lanczos coefficient.
    let values = [2f32.powi(60), -2f32.powi(60)]
        .into_iter()
        .cycle()
        .take(48)
        .collect::<Vec<_>>();
    let source = Image::frombytes("F", (48, 1), &bytes(&values)).expect("wide F source");
    let source_dynamic = source.materialize().expect("materialize wide F source");
    let op = PipelineOp::Resize {
        w: 3,
        h: 1,
        filter: ResampleFilter::Lanczos,
    };
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F"),
    ));
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F"),
    ));

    let expected = bytes(&[
        f32::from_bits(0x5aa1_ab41),
        f32::from_bits(0xc000_0000),
        f32::from_bits(0xdaa1_ab41),
    ]);
    assert_wide_lanczos_cpu_bytes(&source, &expected);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((3, 1), Some(ResampleInput::Name("LANCZOS".into())), None)
        .expect("GPU wide Lanczos resize operation")
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
        Err(error) => panic!("native GPU wide Lanczos resize failed: {error}"),
    };
    assert_eq!(actual, expected, "wide Lanczos native parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("wide Lanczos native resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn assert_wide_lanczos_cpu_bytes(source: &Image, expected: &[u8]) {
    let cpu = source
        .resize((3, 1), Some(ResampleInput::Name("LANCZOS".into())), None)
        .expect("CPU wide Lanczos resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU wide Lanczos resize");
    assert_eq!(cpu, expected);
}

#[test]
fn f_resize_f64_native_preserves_proven_negative_zero_output() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Pillow's ordered f64 Box accumulation leaves a tiny negative
    // residual here; the final f32 store underflows it to signed zero.
    // The exact marker-9 reducer must preserve that sign instead of
    // routing the row back to host control.
    let source_words = [
        f32::from_bits(0x8000_0001).to_bits(),
        0.0f32.to_bits(),
        0.0f32.to_bits(),
    ];
    let source = Image::frombytes("F", (1, 3), &bytes(&source_words)).expect("F source");
    let filter = ResampleInput::Name("BOX".into());
    let source_dynamic = source.materialize().expect("materialize F source");
    assert!(gpu_f_resize_f64_is_exact(
        &[PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        }],
        &source_dynamic,
        Some("F")
    ));
    let expected = source
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU signed-zero resize operation")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU signed-zero F resize");
    assert_eq!(expected, bytes(&[0x8000_0000]));

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((1, 1), Some(filter), None)
        .expect("GPU signed-zero resize operation")
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
        Err(error) => panic!("native GPU signed-zero F resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU signed-zero F resize");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU signed-zero F resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_native_preserves_special_value_outputs() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Marker 9 handles special values in the coefficient reducer without
    // relaxed-f32 arithmetic.  Include a signaling NaN to verify the
    // Pillow/C quieting rule, infinities with both signs, and an
    // opposite-infinity pair that must become the canonical quiet NaN.
    let cases = [
        (
            (2u32, 2u32),
            (1i64, 2i64),
            ResampleInput::Name("BILINEAR".into()),
            vec![0x7fa0_0001, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
        ),
        (
            (2, 2),
            (1, 2),
            ResampleInput::Name("BILINEAR".into()),
            vec![0x7f80_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
        ),
        (
            (2, 2),
            (1, 2),
            ResampleInput::Name("BILINEAR".into()),
            vec![0xff80_0000, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
        ),
        (
            (3, 1),
            (2, 1),
            ResampleInput::Name("BICUBIC".into()),
            vec![0x7f80_0000, 0x3f80_0000, 0xff80_0000],
        ),
        (
            (3, 1),
            (2, 1),
            ResampleInput::Name("BICUBIC".into()),
            vec![0x7fc1_2345, 0x3f80_0000, 0x3f80_0000],
        ),
        (
            (2, 1),
            (6, 1),
            ResampleInput::Name("BOX".into()),
            vec![0x7fa0_0001, 0x3f80_0000],
        ),
        (
            (2, 1),
            (6, 1),
            ResampleInput::Name("BICUBIC".into()),
            vec![0x7f80_0000, 0x3f80_0000],
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (source_dimensions, destination, filter, words) in cases {
        let source = Image::frombytes("F", source_dimensions, &bytes(&words))
            .expect("F special-value source");
        let expected = source
            .resize(destination, Some(filter.clone()), None)
            .expect("CPU special-value resize operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU special-value F resize");
        let actual = match source
            .resize(destination, Some(filter), None)
            .expect("GPU special-value resize operation")
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
            Err(error) => panic!("native GPU special-value F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU special-value F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU special-value F resize must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_wide_special_value_outputs_native() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Marker 12 deliberately stops at 8388607 taps. Marker 9's special
    // prepass is independent of the arm64 vector product/add split, so a
    // 257-tap row can still be native when the host proof agrees on the
    // exact NaN or infinity bits and its encoded table fits one binding.
    // Finite rows use marker 12 when its ordered proof is representable.
    let cases = [
        (
            ResampleFilter::Bilinear,
            0x7fa1_2345u32,
            257u32,
            1u32,
            0usize,
        ),
        (ResampleFilter::Bicubic, 0x7fc2_3456, 257, 1, 0),
        (ResampleFilter::Lanczos, 0x7f80_0000, 257, 1, 0),
        (ResampleFilter::Hamming, 0xff80_0000, 257, 1, 0),
        (ResampleFilter::Box, 0x7f80_0000, 1, 257, 128),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (filter, special, source_w, source_h, special_index) in cases {
        let mut words = vec![0x3f80_0000; (source_w * source_h) as usize];
        words[special_index] = special;
        let source = Image::frombytes("F", (source_w, source_h), &bytes(&words))
            .expect("wide special F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize wide special F source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(gpu_f_resize_f64_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter_name = match filter {
            ResampleFilter::Bilinear => "BILINEAR",
            ResampleFilter::Bicubic => "BICUBIC",
            ResampleFilter::Lanczos => "LANCZOS",
            ResampleFilter::Hamming => "HAMMING",
            ResampleFilter::Box => "BOX",
            _ => unreachable!(),
        };
        let expected = source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("CPU wide special F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide special F bytes");
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU wide special F resize")
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
            Err(error) => panic!("native GPU wide special F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "wide special {filter_name} F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide special F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(
            telemetry.1,
            Backend::Gpu,
            "{filter_name}; fallback reason: {:?}",
            telemetry.7
        );
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
