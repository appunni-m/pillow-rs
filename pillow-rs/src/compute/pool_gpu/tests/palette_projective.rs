use super::*;

#[test]
fn palette_alpha_projective_relocation_proof_is_narrow() {
    assert!(gpu_transform_should_premultiply(
        1,
        None,
        ResampleFilter::Bilinear,
        TransformMethod::Quad,
    ));
    let image = DynamicImage::ImageLuma8(
        GrayImage::from_raw(16, 16, vec![0; 16 * 16]).expect("luma image"),
    );
    let direct = [1.0, 0.0, -2.0, 0.0, 1.0, 1.0, 0.0, 0.0];
    let swapped = [0.0, 1.0, 3.0, 1.0, 0.0, -4.0, 0.0, 0.0];
    let quad_axis_swap = [0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0];
    let quad_direct = [0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0];
    let quad_constant_fractional = [3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0];
    let mesh_relocation = [0.0, 0.0, 9.0, 6.0, 1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0];
    assert_palette_alpha_nearest_proofs(
        &direct,
        &swapped,
        &quad_axis_swap,
        &quad_direct,
        &quad_constant_fractional,
    );
    assert_palette_alpha_filtered_shape_proofs(&image, &quad_constant_fractional, &mesh_relocation);
    assert_palette_alpha_filtered_mode_proofs(
        &direct,
        &swapped,
        &quad_axis_swap,
        &quad_direct,
        &mesh_relocation,
    );
}

fn assert_palette_alpha_nearest_proofs(
    direct: &[f64; 8],
    swapped: &[f64; 8],
    quad_axis_swap: &[f64; 8],
    quad_direct: &[f64; 8],
    quad_constant_fractional: &[f64; 8],
) {
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Nearest,
        (5, 7),
    ));
    let reflected = [-1.0, 0.0, 9.0, 0.0, 1.0, 6.0, 0.0, 0.0];
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        &reflected,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        (9, 6),
    ));
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_axis_swap,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    let quad_constant = [3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        &quad_constant,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_constant_fractional,
        ResampleFilter::Nearest,
        (9, 6),
    ));
}

fn assert_palette_alpha_filtered_shape_proofs(
    image: &DynamicImage,
    quad_constant_fractional: &[f64; 8],
    mesh_relocation: &[f64; 12],
) {
    let quad_constant_half = [1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        &quad_constant_half,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_constant_fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    let mesh_filtered = PipelineOp::Transform {
        w: 9,
        h: 6,
        method: TransformMethod::Mesh,
        data: Arc::from(mesh_relocation.to_vec()),
        filter: ResampleFilter::Bilinear,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_projective_nearest_is_exact(
        &mesh_filtered,
        image,
        Some("L"),
        (16, 16)
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        mesh_relocation,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    let mesh_constant = [0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    let mesh_constant_half = [0.0, 0.0, 9.0, 6.0, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant_half,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    let mesh_constant_extra = [
        0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 0.0, 0.0,
    ];
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant_extra,
        ResampleFilter::Nearest,
        (9, 6),
    ));
}

fn assert_palette_alpha_filtered_mode_proofs(
    direct: &[f64; 8],
    swapped: &[f64; 8],
    quad_axis_swap: &[f64; 8],
    quad_direct: &[f64; 8],
    mesh_relocation: &[f64; 12],
) {
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        &[1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0],
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Bicubic,
        Some("PA"),
        (5, 7),
    ));
    let perspective_constant_half = [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &perspective_constant_half,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    let perspective_constant_fractional = [0.0, 0.0, 1.25, 0.0, 0.0, 4.5, 0.0, 0.0];
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &perspective_constant_fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        Some("LA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Bicubic,
        Some("RGBA"),
        (5, 7),
    ));
    let fractional = [1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0];
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &fractional,
        ResampleFilter::Bilinear,
        Some("LA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_axis_swap,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        mesh_relocation,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
}

#[test]
fn palette_alpha_projective_nearest_native_gpu_preserves_pairs() {
    let source_bytes = (0..7 * 5)
        .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("LA", (7, 5), &source_bytes).expect("LA source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");

    let cases = [
        ((7, 5), [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], None),
        (
            (9, 6),
            [1.0, 0.0, 1.0, 0.0, 1.0, 2.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (9, 6),
            [1.0, 0.0, -2.0, 0.0, 1.0, -1.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (5, 7),
            [0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![17, 203])),
        ),
        (
            (7, 5),
            [-1.0, 0.0, 7.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (7, 5),
            [1.0, 0.0, 0.0, 0.0, -1.0, 5.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (7, 5),
            [-1.0, 0.0, 7.0, 0.0, -1.0, 5.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (7, 5),
            [0.0, -1.0, 5.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (7, 5),
            [0.0, 1.0, 0.0, -1.0, 0.0, 7.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (7, 5),
            [0.0, -1.0, 5.0, -1.0, 0.0, 7.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (size, matrix, fill) in cases {
        let transformed = source
            .transform_public(
                size,
                2,
                Some(TransformData::Affine(matrix.to_vec())),
                0,
                0,
                fill,
            )
            .expect("PA projective transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU PA projective transform");
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
            Err(error) => panic!("native GPU PA projective transform failed: {error}"),
        };
        assert_eq!(actual, expected, "PA projective pair parity for {size:?}");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native PA projective transform must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    let constant_cases = [
        (
            (9, 6),
            3,
            TransformData::Affine(vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0]),
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (9, 6),
            3,
            TransformData::Affine(vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0]),
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (9, 6),
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            )]),
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (9, 6),
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0],
            )]),
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (9, 6),
            3,
            TransformData::Affine(vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0]),
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (9, 6),
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0],
            )]),
            Some(TransformFill::Components(vec![199, 71])),
        ),
    ];
    for (size, method, data, fill) in constant_cases {
        let transformed = source
            .transform_public(size, method, Some(data), 0, 0, fill)
            .expect("PA constant projective transform");
        assert_eq!(transformed.mode().expect("constant PA mode"), "PA");
        assert_eq!(transformed.palette(), Some(vec![10, 20, 30, 40, 50, 60]));
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU PA constant projective transform");
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
            Err(error) => panic!("native GPU PA constant projective transform failed: {error}"),
        };
        assert_eq!(actual, expected, "PA constant projective pair parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native PA constant projective transform must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_alpha_projective_filtered_relocations_native_gpu_preserve_pairs() {
    let cases = [
        (
            (1, 1),
            (1, 1),
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            None,
        ),
        (
            (2, 3),
            (2, 3),
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            None,
        ),
        (
            (4, 3),
            (6, 5),
            [1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (7, 5),
            (5, 7),
            [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (16, 9),
            (18, 11),
            [1.0, 0.0, -2.0, 0.0, 1.0, 1.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![17, 203])),
        ),
        (
            (33, 17),
            (31, 19),
            [0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0, 0.0],
            Some(TransformFill::Components(vec![113, 7])),
        ),
        (
            (16, 16),
            (8, 8),
            [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0],
            None,
        ),
        (
            (16, 16),
            (8, 8),
            [0.0, 0.0, -0.5, 0.0, 0.0, -0.5, 0.0, 0.0],
            Some(TransformFill::Components(vec![17, 203])),
        ),
    ];
    let palette = vec![10, 20, 30, 40, 50, 60];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (source_size, output_size, matrix, fill) in cases {
        let (source_width, source_height) = source_size;
        let source_bytes = (0..source_width * source_height)
            .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
            .collect::<Vec<_>>();
        let mut source = Image::frombytes("LA", source_size, &source_bytes).expect("LA source");
        source.putpalette(&palette, "RGB").expect("PA palette");
        assert_eq!(source.mode().expect("PA mode"), "PA");

        for filter in [(2, ResampleFilter::Bilinear), (3, ResampleFilter::Bicubic)] {
            let transformed = source
                .transform_public(
                    output_size,
                    2,
                    Some(TransformData::Affine(matrix.to_vec())),
                    filter.0,
                    0,
                    fill.clone(),
                )
                .expect("PA filtered projective transform");
            assert_eq!(transformed.mode().expect("transformed PA mode"), "PA");
            assert_eq!(transformed.palette(), Some(palette.clone()));
            let expected_image = transformed.clone().use_backend(Backend::Cpu);
            let expected = expected_image
                .tobytes()
                .expect("CPU PA filtered projective transform");
            let gpu_image = transformed.clone().use_backend(Backend::Gpu);
            assert_eq!(gpu_image.mode().expect("GPU PA mode"), "PA");
            assert_eq!(gpu_image.palette(), Some(palette.clone()));
            let actual = match gpu_image.tobytes() {
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
                    panic!("native GPU PA filtered projective transform failed: {error}")
                }
            };
            assert_eq!(
                actual, expected,
                "PA filtered projective parity for {source_size:?} -> {output_size:?}, filter={:?}",
                filter.1
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native PA filtered projective transform must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_alpha_quad_mesh_filtered_relocations_native_gpu_preserve_pairs() {
    let source_size = (16u32, 16u32);
    let source_bytes = (0..source_size.0 as usize * source_size.1 as usize)
        .flat_map(|index| [(index * 17 + 3) as u8, (index * 29 + 11) as u8])
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("LA", source_size, &source_bytes).expect("LA source");
    let palette = vec![10, 20, 30, 40, 50, 60];
    source.putpalette(&palette, "RGB").expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");

    let cases = [
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0]),
        ),
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0],
            )]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![1.0, 2.0, 7.0, 2.0, 7.0, 11.0, 1.0, 11.0],
            )]),
        ),
        (
            3,
            TransformData::Affine(vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5],
            )]),
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (method, data) in cases {
        for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
            let transformed = source
                .transform_public(
                    (9, 6),
                    method,
                    Some(data.clone()),
                    match filter {
                        ResampleFilter::Bilinear => 2,
                        ResampleFilter::Bicubic => 3,
                        _ => unreachable!(),
                    },
                    0,
                    Some(TransformFill::Components(vec![199, 71])),
                )
                .expect("filtered PA Quad/Mesh relocation");
            assert_eq!(transformed.mode().expect("PA mode"), "PA");
            assert_eq!(transformed.palette(), Some(palette.clone()));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU filtered PA Quad/Mesh relocation");
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
                    panic!("native GPU filtered PA Quad/Mesh relocation failed: {error}")
                }
            };
            assert_eq!(actual, expected, "PA filtered method {method}");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("filtered PA Quad/Mesh relocation must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_alpha_quad_mesh_nearest_native_gpu_preserves_pairs() {
    let source_bytes = (0..8 * 8)
        .flat_map(|index| [(index * 19 + 5) as u8, (index * 31 + 13) as u8])
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("LA", (8, 8), &source_bytes).expect("LA source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");

    let cases = [
        (
            (8, 8),
            3,
            TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
            None,
        ),
        (
            (8, 8),
            3,
            TransformData::Affine(vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0]),
            Some(TransformFill::Components(vec![211, 83])),
        ),
        (
            (10, 10),
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 10.0, 10.0],
                vec![1.0, 2.0, 1.0, 12.0, 11.0, 12.0, 11.0, 2.0],
            )]),
            Some(TransformFill::Components(vec![199, 71])),
        ),
        (
            (8, 8),
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
            )]),
            None,
        ),
        (
            (8, 8),
            3,
            TransformData::Affine(vec![1.0, 2.0, 1.0, 10.0, 9.0, 10.0, 9.0, 2.0]),
            Some(TransformFill::Components(vec![61, 233])),
        ),
        (
            (8, 8),
            3,
            TransformData::Affine(vec![1.0, 2.0, 9.0, 2.0, 9.0, 10.0, 1.0, 10.0]),
            Some(TransformFill::Components(vec![17, 203])),
        ),
    ];

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (size, method, data, fill) in cases {
        let transformed = source
            .transform_public(size, method, Some(data), 0, 0, fill)
            .expect("PA Quad/Mesh transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU PA Quad/Mesh transform");
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
            Err(error) => panic!("native GPU PA Quad/Mesh transform failed: {error}"),
        };
        assert_eq!(actual, expected, "PA method {method} pair parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native PA Quad/Mesh transform must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn fractional_projective_nearest_native_gpu_preserves_oracle() {
    let width = 9;
    let height = 8;
    let source_bytes: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
        .collect();
    let source = Image::frombytes("L", (width, height), &source_bytes).expect("luma source");
    let transformed = source
        .transform_public(
            (8, 7),
            2,
            Some(TransformData::Affine(vec![
                1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002,
            ])),
            0,
            0,
            Some(TransformFill::Scalar(17)),
        )
        .expect("perspective transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU perspective transform");
    let expected_oracle = [
        3, 40, 77, 114, 151, 188, 225, 6, 51, 88, 125, 162, 162, 199, 236, 17, 62, 99, 136, 173,
        210, 247, 28, 65, 73, 110, 147, 184, 221, 2, 39, 76, 84, 121, 158, 195, 232, 13, 50, 87,
        95, 132, 169, 206, 243, 24, 61, 98, 106, 143, 180, 217, 254, 35, 72, 109,
    ];
    assert_eq!(expected, expected_oracle);

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
        Err(error) => panic!("fractional GPU perspective transform failed: {error}"),
    };
    assert_eq!(actual, expected_oracle);
    if let Some(telemetry) = Backend::take_pipeline_telemetry() {
        // The selected backend is the semantic fact this regression
        // protects. Other tests may toggle the process-wide telemetry
        // switch while the harness is running, so tolerate a missing or
        // replaced sample in parallel test execution.
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_alpha_nearest_affine_native_gpu_preserves_pairs() {
    let source_bytes = [
        1u8, 17, 2, 33, 3, 49, 4, 65, 5, 81, 6, 97, 7, 113, 8, 129, 9, 145, 10, 161, 11, 177, 12,
        193,
    ];
    let mut source = Image::frombytes("LA", (4, 3), &source_bytes).expect("LA source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");
    let transformed = source
        .transform_public(
            (4, 3),
            0,
            Some(TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0])),
            0,
            0,
            None,
        )
        .expect("PA affine transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU PA transform");
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
        Err(error) => panic!("native GPU PA affine transform failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU PA affine parity");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU PA affine transform must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn palette_alpha_nearest_rotate_native_gpu_preserves_pairs() {
    let source_bytes = [
        1u8, 17, 2, 33, 3, 49, 4, 65, 5, 81, 6, 97, 7, 113, 8, 129, 9, 145, 10, 161, 11, 177, 12,
        193,
    ];
    let mut source = Image::frombytes("LA", (4, 3), &source_bytes).expect("LA source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");
    let cases = [
        source
            .rotate_with_input(
                37.5,
                RotateResampleInput::Name("NEAREST".into()),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Default,
                RotatePointInput::Default,
                ImageOpsColor::Components(vec![1, 128]),
            )
            .expect("PA nearest rotate"),
        source
            .rotate_with_input(
                17.5,
                RotateResampleInput::Code(0),
                RotateExpandInput::Boolean(true),
                RotatePointInput::Values(vec![1.25, 0.75]),
                RotatePointInput::Values(vec![0.25, -0.5]),
                ImageOpsColor::Components(vec![7, 201]),
            )
            .expect("PA custom nearest rotate"),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for rotated in cases {
        let expected = rotated
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU PA rotate");
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
            Err(error) => panic!("native GPU PA rotate failed: {error}"),
        };
        assert_eq!(actual, expected, "PA rotate parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU PA rotate must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
