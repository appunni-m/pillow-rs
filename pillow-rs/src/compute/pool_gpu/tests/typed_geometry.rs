use super::*;

#[test]
fn fit_nearest_native_gpu_preserves_indexed_modes() {
    let cases = [("P", vec![17u8; 12]), ("PA", vec![17u8; 12])];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, source_bytes) in cases {
        let source = if mode == "PA" {
            let mut indexed = Image::frombytes("P", (4, 3), &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            indexed.putalpha(201).expect("PA source alpha");
            indexed
        } else {
            Image::frombytes(mode, (4, 3), &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"))
        };
        let fitted = crate::ops::imageops::fit(&source, 3, 2, Some("NEAREST"), 0.0, (0.5, 0.5))
            .expect("Fit operation");
        let expected = fitted
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU Fit");
        let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
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
            Err(error) => panic!("native GPU {mode} Fit failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} Fit parity");
        let telemetry =
            Backend::take_pipeline_telemetry().expect("native GPU Fit must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(2));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_fit_nearest_native_gpu_preserves_scalar_words() {
    let values = [
        0.25f32, -1.5, 3.0, 7.25, 11.0, 13.5, 17.75, 19.0, 23.25, 29.5, 31.0, 37.75, 41.0, 43.5,
        47.25, 53.0, 59.5, 61.25, 67.0, 71.75,
    ];
    let source_bytes = values
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::frombytes("F", (5, 4), &source_bytes).expect("F source");
    let fitted = crate::ops::imageops::fit(&source, 3, 2, Some("NEAREST"), 0.0, (0.5, 0.5))
        .expect("F Fit operation");
    let expected = fitted
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F Fit");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU F Fit failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU F nearest Fit parity");
    let telemetry =
        Backend::take_pipeline_telemetry().expect("native GPU F Fit must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);

    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_cover_pad_nearest_native_gpu_preserves_signed_words() {
    let values = [
        -12345i32,
        -7,
        0,
        206,
        17,
        31,
        4096,
        i32::MAX,
        -1,
        73,
        91,
        127,
    ];
    let source_bytes = values
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::frombytes("I", (12, 1), &source_bytes).expect("I source");
    let covered =
        crate::ops::imageops::cover_with_input(&source, 8, 1, Some(ResampleInput::Code(0)))
            .expect("I Cover operation");
    let padded = crate::ops::imageops::pad_with_input(
        &covered,
        10,
        2,
        Some(ResampleInput::Code(0)),
        crate::ops::imageops::ImageOpsColor::Scalar(7),
        crate::ops::imageops::CenteringInput::Default,
    )
    .expect("I Pad operation");
    let expected = padded
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU I Cover/Pad");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match padded.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU I Cover/Pad failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU I Cover/Pad parity");
    let telemetry =
        Backend::take_pipeline_telemetry().expect("native GPU I Cover/Pad must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(5));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn i_filter_nearest_resize_native_gpu_preserves_signed_words() {
    let source_bytes = (0..99i32)
        .map(|index| index.saturating_mul(257).saturating_sub(20_000))
        .flat_map(i32::to_le_bytes)
        .collect::<Vec<_>>();
    let source = Image::frombytes("I", (11, 9), &source_bytes).expect("I source");
    let filter = PipelineOp::Filter3x3 {
        kernel: [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
        scale: 1.0,
        offset: 0.0,
    };
    let filtered = Image::push_op(&source, filter);
    let resized = filtered
        .resize((8, 6), Some(ResampleInput::Code(0)), None)
        .expect("I nearest resize");
    let expected = resized
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU I filter/resize");

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
        Err(error) => panic!("native GPU I filter/resize failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU I filter/resize parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU I filter/resize must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(3));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}
