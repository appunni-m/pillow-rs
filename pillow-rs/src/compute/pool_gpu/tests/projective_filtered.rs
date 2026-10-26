use super::*;

#[test]
fn byte_projective_filtered_relocations_native_gpu_preserve_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels) in [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
    ] {
        let bytes = (0..16 * 16 * channels)
            .map(|index| (index * 29 + 7) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (16, 16), &bytes).expect("byte source");
        let fill = match mode {
            "LA" => Some(TransformFill::Components(vec![199, 71])),
            "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
            _ => None,
        };
        let matrices = [
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0],
            [1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0],
            [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0],
            [0.0, 0.0, 2.5, 0.0, 0.0, 3.5, 0.0, 0.0],
        ];
        for matrix in matrices {
            for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                let transformed = source
                    .transform_public(
                        (8, 8),
                        2,
                        Some(TransformData::Affine(matrix.to_vec())),
                        match filter {
                            ResampleFilter::Bilinear => 2,
                            ResampleFilter::Bicubic => 3,
                            _ => unreachable!(),
                        },
                        0,
                        fill.clone(),
                    )
                    .expect("filtered projective relocation");
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU filtered projective relocation");
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
                    Err(error) => {
                        panic!("native GPU filtered projective relocation failed: {error}")
                    }
                };
                assert_eq!(actual, expected, "native {mode} filtered relocation");
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("filtered relocation must publish a receipt");
                assert_eq!(
                    telemetry.0,
                    Some(Backend::Gpu),
                    "telemetry={telemetry:?} mode={mode} matrix={matrix:?} filter={filter:?}"
                );
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_quad_mesh_filtered_relocations_native_gpu_preserve_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels) in [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
    ] {
        let source_size = (16u32, 16u32);
        let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
            .map(|index| (index * 29 + 7) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, source_size, &bytes).expect("byte source");
        let fill = match mode {
            "LA" => Some(TransformFill::Components(vec![199, 71])),
            "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
            _ => None,
        };
        let cases = [
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5]),
                (9, 6),
            ),
            (
                3,
                TransformData::Affine(vec![2.5, 3.5, 2.5, 3.5, 2.5, 3.5, 2.5, 3.5]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
                )]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0],
                )]),
                (9, 6),
            ),
            (
                4,
                TransformData::Mesh(vec![(
                    vec![0.0, 0.0, 9.0, 6.0],
                    vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5],
                )]),
                (9, 6),
            ),
        ];
        for (method, data, output_size) in cases {
            for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
                let transformed = source
                    .transform_public(
                        output_size,
                        method,
                        Some(data.clone()),
                        match filter {
                            ResampleFilter::Bilinear => 2,
                            ResampleFilter::Bicubic => 3,
                            _ => unreachable!(),
                        },
                        0,
                        fill.clone(),
                    )
                    .expect("filtered Quad/Mesh relocation");
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU filtered Quad/Mesh relocation");
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
                    Err(error) => {
                        panic!("native GPU filtered Quad/Mesh relocation failed: {error}")
                    }
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} filtered method {method} data={data:?} filter={filter:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("filtered Quad/Mesh relocation must publish a receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(
                    telemetry.1,
                    Backend::Gpu,
                    "telemetry={telemetry:?} mode={mode} method={method} data={data:?} filter={filter:?}"
                );
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_partial_mesh_relocations_native_gpu_preserve_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = [
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0],
        ),
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 6.0, 3.0, 6.0, 8.0, 2.0, 8.0],
        ),
        (
            [0.0, 3.0, 9.0, 7.0],
            [1.0, 2.0, 1.0, 6.0, 10.0, 6.0, 10.0, 2.0],
        ),
        (
            [0.0, 3.0, 9.0, 7.0],
            [1.0, 2.0, 5.0, 2.0, 5.0, 11.0, 1.0, 11.0],
        ),
    ];
    for (mode, channels) in [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
    ] {
        let source_size = (16u32, 16u32);
        let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
            .map(|index| (index * 37 + channels * 11 + 5) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, source_size, &bytes).expect("byte source");
        let fill = match mode {
            "L" => Some(TransformFill::Scalar(199)),
            "LA" => Some(TransformFill::Components(vec![199, 71])),
            "RGB" => Some(TransformFill::Components(vec![199, 71, 17])),
            "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
            _ => unreachable!(),
        };
        for (bbox, quad) in cases {
            let data = TransformData::Mesh(vec![(bbox.to_vec(), quad.to_vec())]);
            for filter in [
                (0, ResampleFilter::Nearest),
                (2, ResampleFilter::Bilinear),
                (3, ResampleFilter::Bicubic),
            ] {
                let transformed = source
                    .transform_public((9, 8), 4, Some(data.clone()), filter.0, 0, fill.clone())
                    .unwrap_or_else(|error| {
                        panic!("{mode} partial Mesh transform bbox={bbox:?} quad={quad:?}: {error}")
                    });
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU partial Mesh transform");
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
                    Err(error) => {
                        panic!("native GPU {mode} partial Mesh transform failed: {error}")
                    }
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} partial Mesh filter={filter:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("partial Mesh transform must publish telemetry");
                assert_eq!(
                    telemetry.0,
                    Some(Backend::Gpu),
                    "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                );
                assert_eq!(
                    telemetry.1,
                    Backend::Gpu,
                    "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                );
                assert_eq!(
                    telemetry.6,
                    Some(1),
                    "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                );
                assert_eq!(
                    telemetry.7, None,
                    "telemetry={telemetry:?} mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}"
                );
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_projective_partial_mesh_relocations_native_gpu_preserve_pairs() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = [
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0],
        ),
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 6.0, 3.0, 6.0, 8.0, 2.0, 8.0],
        ),
    ];
    let palette = (0..768)
        .map(|index| (index * 19 + 7) as u8)
        .collect::<Vec<_>>();
    for (mode, channels, fill) in [
        ("P", 1usize, TransformFill::Scalar(251)),
        ("PA", 2usize, TransformFill::Components(vec![199, 71])),
    ] {
        let source_size = (16u32, 16u32);
        let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
            .map(|index| (index * 37 + channels * 11 + 5) as u8)
            .collect::<Vec<_>>();
        let storage_mode = if mode == "PA" { "LA" } else { mode };
        let mut source = Image::frombytes(storage_mode, source_size, &bytes)
            .unwrap_or_else(|error| panic!("{mode} source: {error}"));
        source
            .putpalette(&palette, "RGB")
            .unwrap_or_else(|error| panic!("{mode} palette: {error}"));
        assert_eq!(source.mode().expect("palette mode"), mode);
        for (bbox, quad) in cases {
            let data = TransformData::Mesh(vec![(bbox.to_vec(), quad.to_vec())]);
            for filter in [
                (0, ResampleFilter::Nearest),
                (2, ResampleFilter::Bilinear),
                (3, ResampleFilter::Bicubic),
            ] {
                let transformed = source
                    .transform_public(
                        (9, 8),
                        4,
                        Some(data.clone()),
                        filter.0,
                        0,
                        Some(fill.clone()),
                    )
                    .unwrap_or_else(|error| {
                        panic!("{mode} partial Mesh transform bbox={bbox:?} quad={quad:?}: {error}")
                    });
                let expected = transformed
                    .clone()
                    .use_backend(Backend::Cpu)
                    .tobytes()
                    .expect("CPU partial palette Mesh transform");
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
                    Err(error) => {
                        panic!("native GPU {mode} partial Mesh transform failed: {error}")
                    }
                };
                assert_eq!(
                    actual, expected,
                    "native {mode} partial Mesh filter={filter:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry()
                    .expect("partial palette Mesh transform must publish telemetry");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.6, Some(1));
                assert_eq!(telemetry.7, None);
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn partial_mesh_without_fill_stays_on_exact_host_control() {
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).expect("RGB image"),
    );
    let op = PipelineOp::Transform {
        w: 9,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from([1.0, 2.0, 6.0, 6.0, 2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0].to_vec()),
        filter: ResampleFilter::Bilinear,
        fill: Some((0, 0, 0, 255)),
        fill_is_none: true,
        palette_fill: None,
    };
    assert!(!gpu_projective_nearest_is_exact(
        &op,
        &image,
        Some("RGB"),
        (16, 16),
    ));
}

#[test]
fn projective_filtered_constant_proof_is_narrow() {
    let source = (16, 16);
    let output = (9, 7);
    let perspective = [0.0, 0.0, 3.0, 0.0, 0.0, 5.0, 0.0, 0.0];
    let quad = [3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
    let mesh = [0.0, 0.0, 9.0, 7.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
    for mode in [
        Some("L"),
        Some("RGB"),
        Some("PA"),
        Some("CMYK"),
        Some("HSV"),
        Some("YCbCr"),
        Some("RGBX"),
        Some("RGBa"),
    ] {
        assert_standard_projective_constant_modes(mode, source, output, &perspective, &quad, &mesh);
    }
    assert_alpha_projective_constant_modes(source, output, &perspective);
    assert_projective_constant_source_boundaries(source, output);
    assert_projective_constant_quarter_translation(source, output);
    assert_projective_constant_cmyk_edges(source, output);
    assert_partial_projective_constant_mesh(source, output);
}

fn assert_standard_projective_constant_modes(
    mode: Option<&str>,
    source: (u32, u32),
    output: (u32, u32),
    perspective: &[f64; 8],
    quad: &[f64; 8],
    mesh: &[f64; 12],
) {
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Perspective,
        perspective,
        ResampleFilter::Bilinear,
        mode,
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Quad,
        quad,
        ResampleFilter::Bilinear,
        mode,
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Mesh,
        mesh,
        ResampleFilter::Bilinear,
        mode,
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Perspective,
        perspective,
        ResampleFilter::Bicubic,
        mode,
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Quad,
        quad,
        ResampleFilter::Bicubic,
        mode,
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Mesh,
        mesh,
        ResampleFilter::Bicubic,
        mode,
        source,
        output,
    ));
}

fn assert_alpha_projective_constant_modes(
    source: (u32, u32),
    output: (u32, u32),
    perspective: &[f64; 8],
) {
    for mode in [Some("LA"), Some("RGBA")] {
        assert!(gpu_projective_filtered_constant_is_admitted(
            TransformMethod::Perspective,
            perspective,
            ResampleFilter::Bilinear,
            mode,
            source,
            output,
        ));
        assert!(!gpu_projective_filtered_constant_is_admitted(
            TransformMethod::Perspective,
            perspective,
            ResampleFilter::Bicubic,
            mode,
            source,
            output,
        ));
    }
}

fn assert_projective_constant_source_boundaries(source: (u32, u32), output: (u32, u32)) {
    for (x, y) in [
        (0.0, 5.0),
        (16.0, 5.0),
        (3.0, 0.0),
        (3.0, 16.0),
        (3.03125, 5.0),
    ] {
        let map = [0.0, 0.0, x, 0.0, 0.0, y, 0.0, 0.0];
        assert!(!gpu_projective_filtered_constant_is_admitted(
            TransformMethod::Perspective,
            &map,
            ResampleFilter::Bilinear,
            Some("RGB"),
            source,
            output,
        ));
        assert!(!gpu_projective_filtered_constant_is_admitted(
            TransformMethod::Perspective,
            &map,
            ResampleFilter::Bicubic,
            Some("RGB"),
            source,
            output,
        ));
    }
}

fn assert_projective_constant_quarter_translation(source: (u32, u32), output: (u32, u32)) {
    let quarter_map = [0.0, 0.0, 3.25, 0.0, 0.0, 5.0, 0.0, 0.0];
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Perspective,
        &quarter_map,
        ResampleFilter::Bilinear,
        Some("RGB"),
        source,
        output,
    ));
    assert!(gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Perspective,
        &quarter_map,
        ResampleFilter::Bicubic,
        Some("RGB"),
        source,
        output,
    ));
}

fn assert_projective_constant_cmyk_edges(source: (u32, u32), output: (u32, u32)) {
    for (x, y) in [(1.0, 5.0), (15.0, 5.0), (3.0, 1.0), (3.0, 15.0)] {
        let map = [0.0, 0.0, x, 0.0, 0.0, y, 0.0, 0.0];
        assert!(!gpu_projective_filtered_constant_is_admitted(
            TransformMethod::Perspective,
            &map,
            ResampleFilter::Bicubic,
            Some("CMYK"),
            source,
            output,
        ));
    }
}

fn assert_partial_projective_constant_mesh(source: (u32, u32), output: (u32, u32)) {
    let partial_mesh = [1.0, 1.0, 8.0, 6.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0];
    assert!(!gpu_projective_filtered_constant_is_admitted(
        TransformMethod::Mesh,
        &partial_mesh,
        ResampleFilter::Bilinear,
        Some("RGB"),
        source,
        output,
    ));
}

#[test]
fn projective_filtered_integer_constants_native_gpu_preserve_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let source_size = (16, 16);
    let output_size = (9, 7);
    let maps = integer_constant_projective_maps();
    for (mode, channels) in [
        ("L", 1usize),
        ("RGB", 3usize),
        ("PA", 2usize),
        ("CMYK", 4usize),
        ("HSV", 3usize),
        ("YCbCr", 3usize),
        ("RGBX", 4usize),
        ("RGBa", 4usize),
    ] {
        if !assert_integer_constant_projective_mode(mode, channels, source_size, output_size, &maps)
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn integer_constant_projective_maps() -> [(i32, TransformData); 3] {
    [
        (
            2,
            TransformData::Affine(vec![0.0, 0.0, 3.0, 0.0, 0.0, 5.0, 0.0, 0.0]),
        ),
        (
            3,
            TransformData::Affine(vec![3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 7.0],
                vec![3.0, 5.0, 3.0, 5.0, 3.0, 5.0, 3.0, 5.0],
            )]),
        ),
    ]
}

fn assert_integer_constant_projective_mode(
    mode: &str,
    channels: usize,
    source_size: (u32, u32),
    output_size: (u32, u32),
    maps: &[(i32, TransformData); 3],
) -> bool {
    let bytes = (0..channels.saturating_mul(256))
        .map(|index| index.wrapping_mul(53).wrapping_add(17) as u8)
        .collect::<Vec<_>>();
    let mut source = Image::frombytes(if mode == "PA" { "LA" } else { mode }, source_size, &bytes)
        .expect("integer-constant projective source");
    if mode == "PA" {
        let palette = (0usize..768)
            .map(|index| index.wrapping_mul(19).wrapping_add(7) as u8)
            .collect::<Vec<_>>();
        source
            .putpalette(&palette, "RGB")
            .expect("integer-constant PA palette");
    }
    let fill = match mode {
        "L" => TransformFill::Scalar(199),
        "RGB" => TransformFill::Components(vec![199, 71, 17]),
        "PA" => TransformFill::Components(vec![199, 71]),
        "CMYK" | "RGBX" | "RGBa" => TransformFill::Components(vec![199, 71, 17, 233]),
        "HSV" | "YCbCr" => TransformFill::Components(vec![199, 71, 17]),
        _ => unreachable!(),
    };
    for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
        for (method, data) in maps {
            let transformed = source
                .transform_public(
                    output_size,
                    *method,
                    Some(data.clone()),
                    match filter {
                        ResampleFilter::Bilinear => 2,
                        ResampleFilter::Bicubic => 3,
                        _ => unreachable!(),
                    },
                    0,
                    Some(fill.clone()),
                )
                .expect("integer-constant projective transform");
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU integer-constant projective transform");
            let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    return false;
                }
                Err(error) => {
                    panic!("native GPU integer-constant projective transform failed: {error}")
                }
            };
            assert_eq!(
                actual, expected,
                "native {mode} method={method} filter={filter:?} integer constant"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("integer-constant projective transform must publish telemetry");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
    }
    true
}
