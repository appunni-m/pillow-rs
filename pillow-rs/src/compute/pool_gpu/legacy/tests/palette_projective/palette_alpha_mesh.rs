use super::{Backend, Image, ResampleFilter, TransformData, TransformFill};

type FilteredRelocationCase = (i32, TransformData);
type NearestRelocationCase = ((u32, u32), i32, TransformData, Option<TransformFill>);

#[test]
fn palette_alpha_quad_mesh_filtered_relocations_native_gpu_preserve_pairs() {
    let (source, palette) = filtered_relocation_source();
    let cases = filtered_relocation_cases();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (method, data) in cases {
        for filter in [ResampleFilter::Bilinear, ResampleFilter::Bicubic] {
            if !assert_filtered_relocation_case(&source, &palette, method, &data, filter) {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn filtered_relocation_source() -> (Image, Vec<u8>) {
    let source_size = (16u32, 16u32);
    let pixel_count = (source_size.0 as usize)
        .checked_mul(source_size.1 as usize)
        .expect("filtered PA fixture size");
    let source_bytes = (0..pixel_count)
        .flat_map(|index| {
            [
                index.wrapping_mul(17).wrapping_add(3) as u8,
                index.wrapping_mul(29).wrapping_add(11) as u8,
            ]
        })
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("LA", source_size, &source_bytes).expect("LA source");
    let palette = vec![10, 20, 30, 40, 50, 60];
    source.putpalette(&palette, "RGB").expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");
    (source, palette)
}

fn filtered_relocation_cases() -> [FilteredRelocationCase; 6] {
    [
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
    ]
}

fn assert_filtered_relocation_case(
    source: &Image,
    palette: &[u8],
    method: i32,
    data: &TransformData,
    filter: ResampleFilter,
) -> bool {
    let transformed = source
        .transform_public(
            (9, 6),
            method,
            Some(data.clone()),
            filter_parameter(filter),
            0,
            Some(TransformFill::Components(vec![199, 71])),
        )
        .expect("filtered PA Quad/Mesh relocation");
    assert_eq!(transformed.mode().expect("PA mode"), "PA");
    assert_eq!(transformed.palette(), Some(palette.to_vec()));
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU filtered PA Quad/Mesh relocation");
    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error) if gpu_initialization_unavailable(&error) => return false,
        Err(error) => panic!("native GPU filtered PA Quad/Mesh relocation failed: {error}"),
    };
    assert_eq!(actual, expected, "PA filtered method {method}");
    assert_filtered_relocation_receipt();
    true
}

fn assert_filtered_relocation_receipt() {
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("filtered PA Quad/Mesh relocation must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
}

#[test]
fn palette_alpha_quad_mesh_nearest_native_gpu_preserves_pairs() {
    let source = nearest_relocation_source();
    let cases = nearest_relocation_cases();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (size, method, data, fill) in cases {
        if !assert_nearest_relocation_case(&source, size, method, data, fill) {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn nearest_relocation_source() -> Image {
    let source_bytes = (0usize..64)
        .flat_map(|index| {
            [
                index.wrapping_mul(19).wrapping_add(5) as u8,
                index.wrapping_mul(31).wrapping_add(13) as u8,
            ]
        })
        .collect::<Vec<_>>();
    let mut source = Image::frombytes("LA", (8, 8), &source_bytes).expect("LA source");
    source
        .putpalette(&[10, 20, 30, 40, 50, 60], "RGB")
        .expect("PA palette");
    assert_eq!(source.mode().expect("PA mode"), "PA");
    source
}

fn nearest_relocation_cases() -> [NearestRelocationCase; 6] {
    [
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
    ]
}

fn assert_nearest_relocation_case(
    source: &Image,
    size: (u32, u32),
    method: i32,
    data: TransformData,
    fill: Option<TransformFill>,
) -> bool {
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
        Err(error) if gpu_initialization_unavailable(&error) => return false,
        Err(error) => panic!("native GPU PA Quad/Mesh transform failed: {error}"),
    };
    assert_eq!(actual, expected, "PA method {method} pair parity");
    assert_nearest_relocation_receipt();
    true
}

fn assert_nearest_relocation_receipt() {
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native PA Quad/Mesh transform must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu, "telemetry={telemetry:?}");
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
}

fn gpu_initialization_unavailable(error: &impl std::fmt::Display) -> bool {
    let message = error.to_string();
    message.contains("GPU adapter not available")
        || message.contains("GPU device initialization failed")
}

fn filter_parameter(filter: ResampleFilter) -> i32 {
    match filter {
        ResampleFilter::Bilinear => 2,
        ResampleFilter::Bicubic => 3,
        _ => unreachable!(),
    }
}
