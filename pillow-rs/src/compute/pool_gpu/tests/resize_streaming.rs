use super::*;

#[test]
fn f_resize_f64_special_over_8388607_taps_streams_natively() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Marker 9's special-value prepass must obey the same adapter-fitting
    // coefficient cap as marker 12. Before tiling, an 8388608-tap
    // NaN row bypassed the finite-row limit and reached bind-group
    // creation with a 134217984-byte range, exceeding wgpu's 128-MiB
    // storage binding limit. The tiled path must retain native execution
    // while preserving this NaN payload across transport boundaries.
    let width = 8_388_608usize;
    let mut words = vec![0x3f80_0000; width];
    words[width / 2] = 0x7fc1_2345;
    let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
        .expect("over-bound special F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize over-bound special F source");
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

    let filter = ResampleInput::Name("BILINEAR".into());
    let expected = source
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU over-bound special F resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU over-bound special F bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((1, 1), Some(filter), None)
        .expect("GPU over-bound special F resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native tiled over-bound special F resize");
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("over-bound special F resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_f64_coeff_table_over_binding_streams_natively() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Each output row below is individually within the ordered reducer's
    // tap cap, but the two-row horizontal table is 201326592 bytes after
    // f64 coefficient encoding. Stream the table without relaxing the
    // adapter limit or replacing GPU arithmetic with a host result.
    let width = 8_388_607usize;
    let words: Vec<u32> = (0..width)
        .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
        .collect();
    let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
        .expect("multi-row coefficient F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize multi-row coefficient F source");
    let op = PipelineOp::Resize {
        w: 2,
        h: 1,
        filter: ResampleFilter::Bilinear,
    };
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));

    let filter = ResampleInput::Name("BILINEAR".into());
    let expected = source
        .resize((2, 1), Some(filter.clone()), None)
        .expect("CPU multi-row coefficient F resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU multi-row coefficient F bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((2, 1), Some(filter), None)
        .expect("GPU multi-row coefficient F resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native tiled multi-row coefficient F resize");
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("multi-row coefficient F resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_ordered_f64_subnormal_and_extreme_finite_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // f32 subnormals are still ordinary finite values to Pillow's f64
    // accumulator.  Verify the explicit 2^-149 representation survives
    // the wider marker-12 row, alongside the largest finite f32 word.
    for (filter, value) in [
        (ResampleFilter::Bilinear, 0x0000_0001u32),
        (ResampleFilter::Lanczos, 0x0000_0003u32),
        (ResampleFilter::Box, 0x0000_0007u32),
        (ResampleFilter::Bicubic, 0x7f7f_ffffu32),
        (ResampleFilter::Hamming, 0x7f7f_ffffu32),
    ] {
        let width = 32768usize;
        let words = vec![value; width];
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("subnormal/extreme F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize subnormal/extreme F source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(
            gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ),
            "ordered proof should cover 32768-tap {filter:?} row"
        );
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
            .expect("CPU subnormal/extreme F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU subnormal/extreme F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU subnormal/extreme F resize")
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
            Err(error) => panic!("native GPU subnormal/extreme F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "32768-tap {filter_name} F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("subnormal/extreme F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}

#[test]
fn f_resize_ordered_f64_subnormal_vertical_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // Exercise the separate vertical shader reducer as well as the
    // horizontal path above.  Vertical FLOAT32 resampling stays on the
    // scalar FMA ordering, including when its source words are f32
    // subnormals or largest-finite values.
    for (filter, value) in [
        (ResampleFilter::Bilinear, 0x0000_0001u32),
        (ResampleFilter::Bicubic, 0x0000_0003u32),
        (ResampleFilter::Lanczos, 0x8000_0007u32),
        (ResampleFilter::Hamming, 0x7f7f_ffffu32),
        (ResampleFilter::Box, 0x7f7f_ffffu32),
    ] {
        let height = 32768usize;
        let words = vec![value; height];
        let source = Image::frombytes("F", (1, height as u32), &bytes(&words))
            .expect("vertical subnormal/extreme F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize vertical subnormal/extreme F source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(gpu_f_resize_f64_ordered_is_exact(
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
            .expect("CPU vertical subnormal/extreme F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU vertical subnormal/extreme F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU vertical subnormal/extreme F resize")
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
            Err(error) => panic!("native GPU vertical F resize failed: {error}"),
        };
        assert_eq!(
            actual, expected,
            "32768-tap vertical {filter_name} F resize"
        );
        let telemetry =
            Backend::take_pipeline_telemetry().expect("vertical F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(
            telemetry.1,
            Backend::Gpu,
            "vertical {filter_name}; fallback reason: {:?}",
            telemetry.7
        );
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}

#[test]
fn f_resize_ordered_f64_wide_cancellation_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    for (width, filter) in [
        (65usize, ResampleFilter::Bilinear),
        (96usize, ResampleFilter::Lanczos),
    ] {
        let words: Vec<u32> = (0..width)
            .map(|index| {
                if index % 2 == 0 {
                    0x3f80_0000
                } else {
                    0xbf80_0000
                }
            })
            .collect();
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("wide cancellation F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize cancellation source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(gpu_f_resize_f64_ordered_is_exact(
            std::slice::from_ref(&op),
            &source_dynamic,
            Some("F")
        ));
        let filter_name = match filter {
            ResampleFilter::Bilinear => "BILINEAR",
            ResampleFilter::Lanczos => "LANCZOS",
            _ => unreachable!(),
        };
        let expected = source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("CPU cancellation resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU cancellation bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU cancellation resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("GPU cancellation bytes");
        assert_eq!(actual, expected, "wide cancellation {filter_name} resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide cancellation resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}
