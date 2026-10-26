use super::*;

#[test]
fn transform_safety_is_deferred_to_image_aware_preflight() {
    let op = PipelineOp::Transform {
        w: 2,
        h: 1,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, f64::NAN, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((0, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };

    // The operation-only router runs before the source image is
    // materialized. A non-finite map must therefore reach the GPU pool's
    // image-aware exact-host-control preflight instead of being reported
    // as a public "GPU does not support Transform" capability error.
    assert!(gpu_operation_requires_image_context(&op));
}

#[test]
fn transform_all_fill_proof_tracks_float_and_typed_bounds() {
    let transform = |data: [f64; 6], filter| PipelineOp::Transform {
        w: 6,
        h: 6,
        method: TransformMethod::Affine,
        data: Arc::from(data.to_vec()),
        filter,
        fill: Some((7, 8, 9, 10)),
        fill_is_none: false,
        palette_fill: None,
    };
    let outside = [1.0, 0.0, 100.0, 0.0, 1.0, 100.0];
    let in_bounds = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).unwrap());
    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).unwrap());
    let luma16 = DynamicImage::ImageLuma16(
        ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(16, 16, vec![0; 16 * 16]).unwrap(),
    );

    assert!(gpu_transform_all_fill_is_exact(
        &transform(outside, ResampleFilter::Bicubic),
        &rgba,
        Some("F"),
        (16, 16),
    ));
    assert!(gpu_transform_all_fill_is_exact(
        &transform(outside, ResampleFilter::Bicubic),
        &rgb,
        Some("HSV"),
        (16, 16),
    ));
    assert!(gpu_transform_all_fill_is_exact(
        &transform(outside, ResampleFilter::Bicubic),
        &luma16,
        Some("I;16"),
        (16, 16),
    ));
    assert!(!gpu_transform_all_fill_is_exact(
        &transform(in_bounds, ResampleFilter::Bicubic),
        &rgba,
        Some("F"),
        (16, 16),
    ));
    assert!(!gpu_transform_all_fill_is_exact(
        &PipelineOp::Transform {
            w: 6,
            h: 6,
            method: TransformMethod::Perspective,
            data: Arc::from(vec![1.0; 8]),
            filter: ResampleFilter::Bicubic,
            fill: Some((7, 8, 9, 10)),
            fill_is_none: false,
            palette_fill: None,
        },
        &rgba,
        Some("F"),
        (16, 16),
    ));
}

#[test]
fn float_nearest_affine_proof_matches_fixed_coordinate_selection() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).expect("F backing image"),
    );
    let extent = PipelineOp::Transform {
        w: 6,
        h: 6,
        method: TransformMethod::Affine,
        data: Arc::from(vec![7.0 / 6.0, 0.0, -1.0, 0.0, 7.0 / 6.0, -1.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((76, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_nearest_affine_is_exact(
        &extent,
        &image,
        Some("F"),
        (16, 16)
    ));

    // A sub-16.16 boundary difference must stay host-controlled: Pillow
    // truncates the original f64 coordinate to source pixel 0, while the
    // uploaded fixed-point origin rounds to source pixel 1.
    let boundary = PipelineOp::Transform {
        w: 1,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.499999, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((76, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    let small_image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 2, vec![0; 2 * 2 * 4]).expect("small F backing image"),
    );
    assert!(!gpu_nearest_affine_is_exact(
        &boundary,
        &small_image,
        Some("F"),
        (2, 2)
    ));

    // Filtered F transforms interpolate scalar samples in Pillow; the
    // mode-8 GPU branch is a raw-word relocation and must not be selected
    // merely because the source coordinates happen to be safe.
    let filtered = PipelineOp::Transform {
        w: 1,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Bilinear,
        fill: Some((76, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(!gpu_nearest_affine_is_exact(
        &filtered,
        &small_image,
        Some("F"),
        (2, 2)
    ));

    assert_nearest_affine_no_fill_uses_transparent_fill();
}

fn assert_nearest_affine_no_fill_uses_transparent_fill() {
    let no_fill = PipelineOp::Transform {
        w: 1,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };
    assert_eq!(gpu_transform_fill(&no_fill, Some("F"), 8), 0);
}

#[test]
fn float_nearest_affine_native_gpu_preserves_words() {
    let words = (0..16 * 16)
        .map(|index| (index as f32) + 0.25)
        .collect::<Vec<_>>();
    let bytes = words
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let source = Image::frombytes("F", (16, 16), &bytes).expect("F source");
    let transformed = source
        .transform_public(
            (6, 6),
            1,
            Some(TransformData::Affine(vec![-1.0, -1.0, 6.0, 6.0])),
            0,
            0,
            None,
        )
        .expect("F extent transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F extent transform");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
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
        Err(error) => panic!("native GPU F extent transform failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU F extent parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native F extent transform must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn float_nearest_rotate_native_gpu_preserves_words() {
    let mut values = (0..16 * 16)
        .map(|index| (index as f32) + 0.25)
        .collect::<Vec<_>>();
    values[17] = f32::from_bits(0x8000_0000);
    values[85] = f32::from_bits(0x7fc0_1234);
    values[170] = f32::from_bits(0x7f80_0000);
    values[221] = f32::from_bits(0x0000_0001);
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let source = Image::frombytes("F", (16, 16), &bytes).expect("F source");
    let rotated = source
        .rotate_with_input(
            13.0,
            RotateResampleInput::Name("NEAREST".into()),
            RotateExpandInput::Boolean(true),
            RotatePointInput::Default,
            RotatePointInput::Default,
            ImageOpsColor::None,
        )
        .expect("F nearest rotate");
    let expected = rotated
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU F nearest rotate");

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
        Err(error) => panic!("native GPU F nearest rotate failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU F nearest rotate parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F nearest rotate must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}
