use super::projective_nearest_execution_cases::{
    byte_projective_fractional_cases, byte_projective_integer_affine_cases,
    byte_projective_nearest_byte_modes, byte_projective_nearest_palette_modes,
    byte_projective_nearest_proven_cases, byte_projective_nonconstant_denominator_cases,
    byte_projective_quad_mesh_cases,
};
use super::*;

#[test]
fn indexed_projective_nearest_native_gpu_preserves_index_bytes() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let mut bytes = vec![0u8; 16 * 16];
    bytes[3 * 16 + 2] = 23;
    let source = Image::frombytes("P", (16, 16), &bytes).expect("indexed source");
    let cases = [
        (
            2,
            TransformData::Affine(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
        ),
        (
            3,
            TransformData::Affine(vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 8.0, 8.0],
                vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
            )]),
        ),
    ];
    for (method, data) in cases {
        let transformed = source
            .transform_public(
                (8, 8),
                method,
                Some(data),
                0,
                0,
                Some(TransformFill::Scalar(7)),
            )
            .expect("indexed projective transform");
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU indexed transform");
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
            Err(error) => panic!("native GPU indexed transform failed: {error}"),
        };
        assert_eq!(actual, expected, "indexed transform method {method}");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native indexed transform must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_nearest_proven_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels) in byte_projective_nearest_byte_modes() {
        let bytes = (0..16 * 16 * channels)
            .map(|index| (index * 29 + 7) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (16, 16), &bytes).expect("byte source");
        let cases = byte_projective_nearest_proven_cases();
        for (method, data) in cases {
            let transformed = source
                .transform_public((8, 8), method, Some(data), 0, 0, None)
                .expect("proven projective transform");
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU proven projective transform");
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
                    panic!("native GPU proven projective transform failed: {error}")
                }
            };
            assert_eq!(
                actual, expected,
                "native {mode} proven transform method {method}"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native proven projective transform must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_integer_affine_nearest_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = byte_projective_integer_affine_cases();
    for (mode, channels) in byte_projective_nearest_palette_modes() {
        for (seed, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
            let bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * (29 + 2 * seed) + 7 * channels + seed) as u8)
                .collect::<Vec<_>>();
            let mut source = Image::frombytes(mode, *source_size, &bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            if mode == "P" {
                let palette = (0..768)
                    .map(|index| (index * 11 + 3) as u8)
                    .collect::<Vec<_>>();
                source
                    .putpalette(&palette, "RGB")
                    .unwrap_or_else(|error| panic!("P palette: {error}"));
            }
            let fill = byte_projective_fill(mode, *fill);
            let transformed = source
                .transform_public(
                    *output_size,
                    2,
                    Some(TransformData::Affine(matrix.to_vec())),
                    0,
                    0,
                    fill,
                )
                .unwrap_or_else(|error| panic!("{mode} transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU transform: {error}"));
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
                Err(error) => panic!("native GPU {mode} transform failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "native {mode} integer projective matrix={matrix:?}"
            );
            let telemetry = Backend::take_pipeline_telemetry()
                .unwrap_or_else(|| panic!("native GPU {mode} transform missing telemetry"));
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_fractional_nearest_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = byte_projective_fractional_cases();
    for (mode, channels) in byte_projective_nearest_palette_modes() {
        for (case_index, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
            let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * (31 + case_index * 7) + 13 + channels) as u8)
                .collect::<Vec<_>>();
            let mut source = Image::frombytes(mode, *source_size, &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            if mode == "P" {
                let palette = (0..768)
                    .map(|index| (index * 17 + 5) as u8)
                    .collect::<Vec<_>>();
                source
                    .putpalette(&palette, "RGB")
                    .unwrap_or_else(|error| panic!("P palette: {error}"));
            }
            let fill = byte_projective_fill(mode, *fill);
            let transformed = source
                .transform_public(
                    *output_size,
                    2,
                    Some(TransformData::Affine(matrix.to_vec())),
                    0,
                    0,
                    fill,
                )
                .unwrap_or_else(|error| panic!("{mode} fractional transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} fractional CPU transform: {error}"));
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
                Err(error) => panic!("native GPU {mode} fractional transform failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "native {mode} fractional matrix={matrix:?}"
            );
            let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                panic!("native GPU {mode} fractional transform missing telemetry")
            });
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn byte_projective_fill(mode: &str, fill: u8) -> Option<TransformFill> {
    match mode {
        "L" | "P" => Some(TransformFill::Scalar(i64::from(fill))),
        "LA" => Some(TransformFill::Components(vec![i64::from(fill), 93])),
        "RGB" => Some(TransformFill::Components(vec![i64::from(fill), 93, 41])),
        "RGBA" => Some(TransformFill::Components(vec![
            i64::from(fill),
            93,
            41,
            173,
        ])),
        _ => unreachable!(),
    }
}

#[test]
fn byte_projective_nonconstant_denominator_nearest_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = byte_projective_nonconstant_denominator_cases();
    for (mode, channels) in byte_projective_nearest_byte_modes() {
        for (case_index, (source_size, output_size, matrix, fill)) in cases.iter().enumerate() {
            let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
                .map(|index| (index * (37 + case_index * 11) + 13 + channels) as u8)
                .collect::<Vec<_>>();
            let proof_image = match mode {
                "L" => DynamicImage::ImageLuma8(
                    GrayImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                        .expect("L proof image"),
                ),
                "LA" => DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                        .expect("LA proof image"),
                ),
                "RGB" => DynamicImage::ImageRgb8(
                    RgbImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                        .expect("RGB proof image"),
                ),
                "RGBA" => DynamicImage::ImageRgba8(
                    RgbaImage::from_raw(source_size.0, source_size.1, source_bytes.clone())
                        .expect("RGBA proof image"),
                ),
                _ => unreachable!(),
            };
            let proof_op = PipelineOp::Transform {
                w: output_size.0,
                h: output_size.1,
                method: TransformMethod::Perspective,
                data: Arc::from(matrix.to_vec()),
                filter: ResampleFilter::Nearest,
                fill: Some((*fill, 93, 41, 173)),
                fill_is_none: false,
                palette_fill: None,
            };
            assert!(
                gpu_projective_nearest_is_exact(&proof_op, &proof_image, Some(mode), *source_size,),
                "proof rejected case={case_index} mode={mode} matrix={matrix:?}"
            );
            let source = Image::frombytes(mode, *source_size, &source_bytes)
                .unwrap_or_else(|error| panic!("{mode} source: {error}"));
            let fill = match mode {
                "L" => Some(TransformFill::Scalar(i64::from(*fill))),
                "LA" => Some(TransformFill::Components(vec![i64::from(*fill), 93])),
                "RGB" => Some(TransformFill::Components(vec![i64::from(*fill), 93, 41])),
                "RGBA" => Some(TransformFill::Components(vec![
                    i64::from(*fill),
                    93,
                    41,
                    173,
                ])),
                _ => unreachable!(),
            };
            let transformed = source
                .transform_public(
                    *output_size,
                    2,
                    Some(TransformData::Affine(matrix.to_vec())),
                    0,
                    0,
                    fill,
                )
                .unwrap_or_else(|error| panic!("{mode} nonconstant transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU nonconstant transform: {error}"));
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
                Err(error) => panic!("native GPU {mode} nonconstant transform failed: {error}"),
            };
            assert_eq!(actual, expected, "native {mode} matrix={matrix:?}");
            let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                panic!("native GPU {mode} nonconstant transform missing telemetry")
            });
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(
                telemetry.1,
                Backend::Gpu,
                "{mode} actual backend case={case_index} matrix={matrix:?} telemetry={telemetry:?}"
            );
            assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_quad_mesh_nearest_proof_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = byte_projective_quad_mesh_cases();
    for (mode, channels) in byte_projective_nearest_byte_modes() {
        let source_size = (16u32, 16u32);
        let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
            .map(|index| (index * 29 + 7 + channels) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, source_size, &source_bytes)
            .unwrap_or_else(|error| panic!("{mode} source: {error}"));
        let fill = match mode {
            "LA" => Some(TransformFill::Components(vec![199, 71])),
            "RGBA" => Some(TransformFill::Components(vec![199, 71, 17, 83])),
            "RGB" => Some(TransformFill::Components(vec![199, 71, 17])),
            _ => Some(TransformFill::Scalar(199)),
        };
        for (method, data, output_size) in &cases {
            let transformed = source
                .transform_public(
                    *output_size,
                    *method,
                    Some(data.clone()),
                    0,
                    0,
                    fill.clone(),
                )
                .unwrap_or_else(|error| panic!("{mode} projective transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} CPU projective transform: {error}"));
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
                Err(error) => panic!("native GPU {mode} projective transform failed: {error}"),
            };
            assert_eq!(
                actual, expected,
                "native {mode} method={method} data={data:?}"
            );
            let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                panic!("native GPU {mode} projective transform missing telemetry")
            });
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(1));
            assert_eq!(telemetry.7, None);
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn byte_projective_fractional_constant_quad_mesh_native_gpu_preserves_pixels() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = [
        (
            3,
            TransformData::Affine(vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0]),
        ),
        (
            4,
            TransformData::Mesh(vec![(
                vec![0.0, 0.0, 9.0, 6.0],
                vec![3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0],
            )]),
        ),
    ];
    for (mode, channels) in [
        ("L", 1usize),
        ("LA", 2usize),
        ("RGB", 3usize),
        ("RGBA", 4usize),
        ("P", 1usize),
    ] {
        let source_size = (9u32, 6u32);
        let source_bytes = (0..source_size.0 as usize * source_size.1 as usize * channels)
            .map(|index| (index * 73 + 17 + channels) as u8)
            .collect::<Vec<_>>();
        let mut source = Image::frombytes(mode, source_size, &source_bytes)
            .unwrap_or_else(|error| panic!("{mode} source: {error}"));
        if mode == "P" {
            let palette = (0..768)
                .map(|index| (index * 19 + 7) as u8)
                .collect::<Vec<_>>();
            source
                .putpalette(&palette, "RGB")
                .unwrap_or_else(|error| panic!("P palette: {error}"));
        }
        let fill = match mode {
            "L" | "P" => Some(TransformFill::Scalar(251)),
            "LA" => Some(TransformFill::Components(vec![251, 93])),
            "RGB" => Some(TransformFill::Components(vec![251, 93, 41])),
            "RGBA" => Some(TransformFill::Components(vec![251, 93, 41, 173])),
            _ => unreachable!(),
        };
        for (method, data) in &cases {
            let transformed = source
                .transform_public((9, 6), *method, Some(data.clone()), 0, 0, fill.clone())
                .unwrap_or_else(|error| panic!("{mode} constant transform: {error}"));
            let expected = transformed
                .clone()
                .use_backend(Backend::Cpu)
                .tobytes()
                .unwrap_or_else(|error| panic!("{mode} constant CPU transform: {error}"));
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
                Err(error) => panic!("native GPU {mode} constant transform failed: {error}"),
            };
            assert_eq!(actual, expected, "native {mode} constant data={data:?}");
            let telemetry = Backend::take_pipeline_telemetry().unwrap_or_else(|| {
                panic!("native GPU {mode} constant transform missing telemetry")
            });
            assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
            assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
            assert_eq!(telemetry.6, Some(1), "{mode} dispatch count");
            assert_eq!(telemetry.7, None, "{mode} fallback reason");
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn indexed_projective_fractional_nonconstant_nearest_native_gpu_preserves_pixels() {
    let source_size = (9u32, 8u32);
    let source_bytes = (0..source_size.0 as usize * source_size.1 as usize)
        .map(|index| (index * 73 + 19) as u8)
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("P", source_size, &source_bytes).expect("P source");
    let palette = (0..768)
        .map(|index| (index * 23 + 11) as u8)
        .collect::<Vec<_>>();
    source.putpalette(&palette, "RGB").expect("P palette");
    let matrix = [1.0, 0.0, 8.0, 0.0, 1.0, 4.0, -0.125, 0.015625];
    let transformed = source
        .transform_public(
            (8, 7),
            2,
            Some(TransformData::Affine(matrix.to_vec())),
            0,
            0,
            Some(TransformFill::Scalar(251)),
        )
        .expect("P nonconstant projective transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("P nonconstant CPU transform");
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
        Err(error) => panic!("native GPU P nonconstant transform failed: {error}"),
    };
    assert_eq!(actual, expected, "native P nonconstant fractional matrix");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native P nonconstant transform must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}
