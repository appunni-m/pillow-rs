use super::*;

#[test]
fn f_resize_ordered_f64_over_64_taps_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    // The ordered reducer is a dynamic tap loop; exercise rows larger
    // than the former policy cap with heterogeneous finite values. The
    // host proof must still reject any state that cannot be represented,
    // so this test only asserts native execution for rows it certifies.
    for (width, filter) in [
        (65usize, ResampleFilter::Bilinear),
        (96usize, ResampleFilter::Bicubic),
        (128usize, ResampleFilter::Lanczos),
        (96usize, ResampleFilter::Hamming),
        (128usize, ResampleFilter::Box),
    ] {
        let words: Vec<u32> = (0..width)
            .map(|index| {
                let value = 0.5f32 + ((index * 37 % 100) as f32) * 0.01f32;
                value.to_bits()
            })
            .collect();
        let source =
            Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("wide F source");
        let source_dynamic = source.materialize().expect("materialize wide F source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(
            gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ),
            "ordered proof should cover {width}-tap {filter:?} row"
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
            .expect("CPU wide F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU wide F resize")
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
        assert_eq!(actual, expected, "wide {filter_name} F resize");
        let telemetry =
            Backend::take_pipeline_telemetry().expect("wide F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}

#[test]
fn f_resize_ordered_f64_two_axes_over_64_taps_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let width = 65usize;
    let height = 65usize;
    let words: Vec<u32> = (0..width * height)
        .map(|index| {
            let value = 0.75f32 + ((index * 17 % 80) as f32) * 0.01f32;
            value.to_bits()
        })
        .collect();
    let source = Image::frombytes("F", (width as u32, height as u32), &bytes(&words))
        .expect("wide two-axis F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize wide two-axis F source");
    let op = PipelineOp::Resize {
        w: 1,
        h: 1,
        filter: ResampleFilter::Bilinear,
    };
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    let filter = ResampleInput::Name("BILINEAR".into());
    let expected = source
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU wide two-axis F resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU wide two-axis F bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .resize((1, 1), Some(filter), None)
        .expect("GPU wide two-axis F resize")
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
        Err(error) => panic!("native GPU wide two-axis F resize failed: {error}"),
    };
    assert_eq!(actual, expected, "wide two-axis F resize");
    let telemetry =
        Backend::take_pipeline_telemetry().expect("wide two-axis F resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_ordered_f64_through_8388607_taps_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    for (width, filter) in [
        (129usize, ResampleFilter::Bilinear),
        (192usize, ResampleFilter::Bicubic),
        (256usize, ResampleFilter::Lanczos),
        (192usize, ResampleFilter::Hamming),
        (256usize, ResampleFilter::Box),
        (384usize, ResampleFilter::Bilinear),
        (512usize, ResampleFilter::Lanczos),
        (768usize, ResampleFilter::Bicubic),
        (1024usize, ResampleFilter::Box),
        (1536usize, ResampleFilter::Hamming),
        (2048usize, ResampleFilter::Bilinear),
        (3072usize, ResampleFilter::Lanczos),
        (4096usize, ResampleFilter::Box),
        (8192usize, ResampleFilter::Bilinear),
        (8192usize, ResampleFilter::Bicubic),
        (8192usize, ResampleFilter::Lanczos),
        (8192usize, ResampleFilter::Hamming),
        (8192usize, ResampleFilter::Box),
        (16384usize, ResampleFilter::Bilinear),
        (16384usize, ResampleFilter::Bicubic),
        (16384usize, ResampleFilter::Lanczos),
        (16384usize, ResampleFilter::Hamming),
        (16384usize, ResampleFilter::Box),
        (32768usize, ResampleFilter::Bilinear),
        (32768usize, ResampleFilter::Bicubic),
        (32768usize, ResampleFilter::Lanczos),
        (32768usize, ResampleFilter::Hamming),
        (32768usize, ResampleFilter::Box),
        (32769usize, ResampleFilter::Bilinear),
        (32769usize, ResampleFilter::Bicubic),
        (32769usize, ResampleFilter::Lanczos),
        (32769usize, ResampleFilter::Hamming),
        (32769usize, ResampleFilter::Box),
        (65536usize, ResampleFilter::Bilinear),
        (65536usize, ResampleFilter::Bicubic),
        (65536usize, ResampleFilter::Lanczos),
        (65536usize, ResampleFilter::Hamming),
        (65536usize, ResampleFilter::Box),
        (131072usize, ResampleFilter::Bilinear),
        (131072usize, ResampleFilter::Bicubic),
        (131072usize, ResampleFilter::Lanczos),
        (131072usize, ResampleFilter::Hamming),
        (131072usize, ResampleFilter::Box),
        (262144usize, ResampleFilter::Bilinear),
        (262144usize, ResampleFilter::Bicubic),
        (262144usize, ResampleFilter::Lanczos),
        (262144usize, ResampleFilter::Hamming),
        (262144usize, ResampleFilter::Box),
        (524288usize, ResampleFilter::Bilinear),
        (524288usize, ResampleFilter::Bicubic),
        (524288usize, ResampleFilter::Lanczos),
        (524288usize, ResampleFilter::Hamming),
        (524288usize, ResampleFilter::Box),
        (1_048_576usize, ResampleFilter::Bilinear),
        (1_048_576usize, ResampleFilter::Bicubic),
        (1_048_576usize, ResampleFilter::Lanczos),
        (1_048_576usize, ResampleFilter::Hamming),
        (1_048_576usize, ResampleFilter::Box),
        (2_097_152usize, ResampleFilter::Bilinear),
        (2_097_152usize, ResampleFilter::Bicubic),
        (2_097_152usize, ResampleFilter::Lanczos),
        (2_097_152usize, ResampleFilter::Hamming),
        (2_097_152usize, ResampleFilter::Box),
        (4_194_304usize, ResampleFilter::Bilinear),
        (4_194_304usize, ResampleFilter::Bicubic),
        (4_194_304usize, ResampleFilter::Lanczos),
        (4_194_304usize, ResampleFilter::Hamming),
        (4_194_304usize, ResampleFilter::Box),
        (8_388_607usize, ResampleFilter::Bilinear),
    ] {
        let words: Vec<u32> = (0..width)
            .map(|index| {
                let value = 0.25f32 + ((index * 29 % 120) as f32) * 0.01f32;
                value.to_bits()
            })
            .collect();
        let source = Image::frombytes("F", (width as u32, 1), &bytes(&words))
            .expect("wide ordered F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize wide ordered source");
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(
            gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ),
            "ordered proof should cover {width}-tap {filter:?} row"
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
            .expect("CPU wide ordered F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide ordered F bytes");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let actual = match source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU wide ordered F resize")
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
            Err(error) => panic!("native GPU wide ordered F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "{width}-tap {filter_name} F resize");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide ordered F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{width}-tap {filter_name}");
        assert_eq!(telemetry.1, Backend::Gpu, "{width}-tap {filter_name}");
        assert_eq!(telemetry.7, None, "{width}-tap {filter_name}");
        Backend::set_pipeline_telemetry_enabled(previous);
    }
}

#[test]
fn f_resize_ordered_f64_2097152_two_axis_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let width = 2_097_152usize;
    let height = 2usize;
    let words: Vec<u32> = (0..width * height)
        .map(|index| {
            let value = 0.25f32 + ((index * 37 % 180) as f32) * 0.005f32;
            value.to_bits()
        })
        .collect();
    let source = Image::frombytes("F", (width as u32, height as u32), &bytes(&words))
        .expect("wide two-axis ordered F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize wide two-axis ordered source");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for filter in [
        ResampleFilter::Bilinear,
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Hamming,
        ResampleFilter::Box,
    ] {
        let op = PipelineOp::Resize { w: 1, h: 1, filter };
        assert!(
            gpu_f_resize_f64_ordered_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ),
            "ordered proof should cover two-axis {filter:?} resize"
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
            .expect("CPU wide two-axis ordered F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU wide two-axis ordered F bytes");
        let actual = source
            .resize((1, 1), Some(ResampleInput::Name(filter_name.into())), None)
            .expect("GPU wide two-axis ordered F resize")
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("GPU wide two-axis ordered F bytes");
        assert_eq!(
            actual, expected,
            "2097152-tap two-axis {filter_name} resize"
        );
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("wide two-axis ordered F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        // Box retains the exact fixed-table path. Other filters stream
        // 128 horizontal chunks plus one store, then one vertical chunk
        // and its final store.
        assert_eq!(
            telemetry.6,
            Some(if matches!(filter, ResampleFilter::Box) {
                2
            } else {
                131
            })
        );
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_ordered_f64_524289_taps_native_matches_cpu() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let width = 524289usize;
    let words: Vec<u32> = (0..width)
        .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
        .collect();
    let source =
        Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("over-bound F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize over-bound F source");
    let op = PipelineOp::Resize {
        w: 1,
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
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU over-bound F resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU over-bound F bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((1, 1), Some(filter), None)
        .expect("GPU over-bound F resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native 524289-tap F resize");
    assert_eq!(actual, expected);
    let telemetry =
        Backend::take_pipeline_telemetry().expect("524289-tap F resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_ordered_f64_over_8388607_taps_streams_natively() {
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    let width = 8_388_608usize;
    let words: Vec<u32> = (0..width)
        .map(|index| (0.5f32 + ((index * 13 % 90) as f32) * 0.01f32).to_bits())
        .collect();
    let source =
        Image::frombytes("F", (width as u32, 1), &bytes(&words)).expect("over-bound F source");
    let source_dynamic = source
        .materialize()
        .expect("materialize over-bound F source");
    let op = PipelineOp::Resize {
        w: 1,
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
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU over-bound F resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU over-bound F bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((1, 1), Some(filter), None)
        .expect("GPU over-bound F resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native tiled over-bound F resize");
    assert_eq!(actual, expected);
    let telemetry =
        Backend::take_pipeline_telemetry().expect("over-bound F resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}
