use super::*;

#[test]
fn extract_band_native_gpu_handles_4096_square_la_alpha() {
    const WIDTH: u32 = 4096;
    const HEIGHT: u32 = 4096;
    let pixel_count = usize::try_from(WIDTH)
        .expect("width fits usize")
        .checked_mul(usize::try_from(HEIGHT).expect("height fits usize"))
        .expect("4096-square pixel count fits usize");
    let capacity = pixel_count
        .checked_mul(2)
        .expect("LA source byte count fits usize");
    let mut source_bytes = Vec::with_capacity(capacity);
    for index in 0..pixel_count {
        let sample = u8::try_from(index % 256).expect("remainder fits u8");
        source_bytes.push(sample.wrapping_mul(7).wrapping_add(13));
        source_bytes.push(sample.wrapping_mul(37).wrapping_add(53));
    }
    let source =
        Image::frombytes("LA", (WIDTH, HEIGHT), &source_bytes).expect("4096-square LA source");
    let expected = source
        .getchannel(1)
        .expect("LA alpha channel")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU LA alpha extraction");

    // The telemetry switch is process-wide and other GPU tests toggle it.
    // Discard this thread's CPU reference receipt before requesting GPU.
    let _ = Backend::take_pipeline_telemetry();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match source
        .getchannel(1)
        .expect("LA alpha channel")
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
        Err(error) => panic!("4096-square GPU LA alpha extraction failed: {error}"),
    };
    assert_eq!(actual, expected, "4096-square LA alpha extraction parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("4096-square GPU channel extraction must publish a receipt");
    assert_eq!(
        telemetry.0,
        Some(Backend::Gpu),
        "4096-square request unexpectedly fell back: {telemetry:?}"
    );
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    let resources = telemetry
        .8
        .expect("4096-square GPU extraction must publish resource telemetry");
    assert_eq!(resources.upload_bytes, (pixel_count * 2) as u64);
    assert_eq!(resources.readback_bytes, pixel_count as u64);
    assert_eq!(resources.mode_conversion_count, 0);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn effect_spread_native_gpu_preserves_raw_byte_modes() {
    let cases = [
        ("1", vec![0x80, 0x40]),
        ("L", vec![7, 8]),
        ("LA", vec![7, 9, 8, 10]),
        ("P", vec![17, 18]),
        ("RGB", vec![1, 2, 3, 4, 5, 6]),
        ("RGBa", vec![1, 2, 3, 4, 5, 6, 7, 8]),
        ("RGBX", vec![11, 12, 13, 14, 15, 16, 17, 18]),
        ("RGBA", vec![21, 22, 23, 24, 25, 26, 27, 28]),
        ("CMYK", vec![31, 32, 33, 34, 35, 36, 37, 38]),
        ("HSV", vec![41, 42, 43, 44, 45, 46]),
        ("YCbCr", vec![51, 52, 53, 54, 55, 56]),
        (
            "I",
            (-123i32)
                .to_le_bytes()
                .into_iter()
                .chain(456i32.to_le_bytes())
                .collect(),
        ),
        (
            "F",
            1.25f32
                .to_le_bytes()
                .into_iter()
                .chain(2.5f32.to_le_bytes())
                .collect(),
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, source_bytes) in cases {
        let source = Image::frombytes(mode, (2, 1), &source_bytes).expect("source image");
        let expected = crate::image_effect_spread(&source, 0)
            .expect("effect_spread operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU effect_spread");
        let actual = match crate::image_effect_spread(&source, 0)
            .expect("effect_spread operation")
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
            Err(error) => panic!("native GPU {mode} effect_spread failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} effect_spread parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU effect_spread must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn contrast_after_putpixel_native_gpu_uses_current_image_midpoint() {
    let cases = [
        ("L", vec![10, 20, 30, 40]),
        ("LA", vec![10, 200, 20, 180, 30, 160, 40, 140]),
        (
            "RGB",
            vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120],
        ),
        (
            "RGBA",
            vec![
                10, 20, 30, 200, 40, 50, 60, 180, 70, 80, 90, 160, 100, 110, 120, 140,
            ],
        ),
        (
            "CMYK",
            vec![
                10, 20, 30, 40, 40, 50, 60, 80, 70, 80, 90, 160, 100, 110, 120, 140,
            ],
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, bytes) in cases {
        let mut source = Image::frombytes(mode, (2, 2), &bytes)
            .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
        source
            .putpixel(1, 0, 180, 90, 60, 220)
            .unwrap_or_else(|error| panic!("{mode} PutPixel: {error}"));
        let expected = source
            .enhance_contrast(0.5)
            .expect("Contrast operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("{mode} CPU Contrast: {error}"));
        let actual = match source
            .enhance_contrast(0.5)
            .expect("Contrast operation")
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
            Err(error) => panic!("native GPU {mode} Contrast failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} Contrast parity");
        let telemetry =
            Backend::take_pipeline_telemetry().expect("native GPU Contrast must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
        assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
        assert_eq!(telemetry.6, Some(2), "{mode} dispatch count");
        assert_eq!(telemetry.7, None, "{mode} fallback reason");
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn contrast_kernels_match_constructor_blend_for_varied_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels) in [("L", 1), ("LA", 2), ("RGB", 3), ("RGBA", 4), ("CMYK", 4)] {
        let bytes: Vec<u8> = (0..17 * 11 * channels)
            .map(|i| ((i * 73 + (i / 17) * 11 + 29) & 255) as u8)
            .collect();
        let source = Image::frombytes(mode, (17, 11), &bytes).expect("varied source");
        let base = source.contrast_degenerate().expect("reference base");
        for factor in [0.1, 0.3, 0.7, 0.99999999, 1.00000001, 1.2, -1.0, 3.33333] {
            let expected = crate::ops::module_fns::blend(&base, &source, factor)
                .expect("reference blend")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("reference bytes");
            for backend in [Backend::Cpu, Backend::Simd, Backend::Gpu] {
                Backend::take_pipeline_telemetry();
                let actual = source
                    .enhance_contrast(factor)
                    .expect("native contrast")
                    .use_backend(backend)
                    .tobytes();
                let actual = match actual {
                    Ok(bytes) => bytes,
                    Err(error)
                        if backend == Backend::Gpu
                            && (error.to_string().contains("GPU adapter not available")
                                || error
                                    .to_string()
                                    .contains("GPU device initialization failed")) =>
                    {
                        continue;
                    }
                    Err(error) => panic!("{mode} factor={factor} {backend:?}: {error}"),
                };
                assert_eq!(actual, expected, "{mode} factor={factor} {backend:?}");
                let receipt = Backend::take_pipeline_telemetry().expect("native receipt");
                assert_eq!(receipt.1, backend, "{mode} factor={factor}");
                assert_eq!(receipt.7, None, "{mode} factor={factor}");
                if backend == Backend::Gpu {
                    assert_eq!(receipt.6, Some(1));
                }
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn cmyk_putalpha_native_gpu_promotes_exactly() {
    let source_bytes = vec![
        10, 20, 30, 40, // C/M/Y/K
        80, 90, 100, 110,
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);

    let mut scalar = Image::frombytes("CMYK", (2, 1), &source_bytes).expect("CMYK source");
    scalar.putalpha(173).expect("CMYK scalar PutAlpha");
    let expected = scalar
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU CMYK scalar PutAlpha");
    let actual = match scalar.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU CMYK scalar PutAlpha failed: {error}"),
    };
    assert_eq!(actual, expected, "CMYK scalar PutAlpha parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU CMYK scalar PutAlpha must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);

    let mask = Image::frombytes("L", (2, 1), &[7, 231]).expect("L alpha mask");
    let mut masked = Image::frombytes("CMYK", (2, 1), &source_bytes).expect("CMYK source");
    masked.putalpha_data(&mask).expect("CMYK image PutAlpha");
    let expected = masked
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU CMYK image PutAlpha");
    let actual = match masked.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU CMYK image PutAlpha failed: {error}"),
    };
    assert_eq!(actual, expected, "CMYK image PutAlpha parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU CMYK image PutAlpha must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);

    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn draw_point_native_gpu_preserves_raw_byte_modes() {
    let cases = [
        ("1", vec![0x00, 0xff]),
        ("P", vec![17, 18]),
        ("PA", vec![17, 18]),
        ("RGBX", vec![1, 2, 3, 4, 5, 6, 7, 8]),
        ("RGBa", vec![11, 12, 13, 14, 15, 16, 17, 18]),
        ("CMYK", vec![21, 22, 23, 24, 25, 26, 27, 28]),
        ("HSV", vec![31, 32, 33, 34, 35, 36]),
        ("YCbCr", vec![41, 42, 43, 44, 45, 46]),
        (
            "I",
            (-123i32)
                .to_le_bytes()
                .into_iter()
                .chain(456i32.to_le_bytes())
                .collect(),
        ),
        (
            "F",
            1.25f32
                .to_le_bytes()
                .into_iter()
                .chain(2.5f32.to_le_bytes())
                .collect(),
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, source_bytes) in cases {
        let source = if mode == "PA" {
            let mut indexed = Image::frombytes("P", (2, 1), &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"));
            indexed.putalpha(201).expect("PA source alpha");
            indexed
        } else {
            Image::frombytes(mode, (2, 1), &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source image: {error}"))
        };
        let op = PipelineOp::DrawPoint {
            points: Arc::from([(0, 0), (1, 0)]),
            fill: (165, 90, 60, 195),
            alpha_blend_rgb: false,
        };
        let expected = Image::push_op(&source, op.clone())
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU draw point");
        let actual = match Image::push_op(&source, op)
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
            Err(error) => panic!("native GPU {mode} draw point failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} draw point parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU draw point must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
