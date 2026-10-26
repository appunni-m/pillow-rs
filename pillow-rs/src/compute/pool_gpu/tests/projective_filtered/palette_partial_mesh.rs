use super::{
    Backend, Image, ResampleFilter, TransformData, TransformFill, assert_projective_gpu_parity_case,
};

#[test]
fn palette_projective_partial_mesh_relocations_native_gpu_preserve_pairs() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let cases = palette_partial_mesh_relocation_cases();
    let palette = (0..768)
        .map(|index| (index * 19 + 7) as u8)
        .collect::<Vec<_>>();
    for (mode, channels, fill) in [
        ("P", 1usize, TransformFill::Scalar(251)),
        ("PA", 2usize, TransformFill::Components(vec![199, 71])),
    ] {
        let source = palette_partial_mesh_source(mode, channels, &palette);
        if !assert_palette_partial_mesh_mode(&source, mode, &fill, &cases, previous) {
            return;
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

type PalettePartialMeshCase = ([f64; 4], [f64; 8]);

fn palette_partial_mesh_relocation_cases() -> [PalettePartialMeshCase; 2] {
    [
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 2.0, 7.0, 7.0, 7.0, 7.0, 3.0],
        ),
        (
            [1.0, 2.0, 6.0, 6.0],
            [2.0, 3.0, 6.0, 3.0, 6.0, 8.0, 2.0, 8.0],
        ),
    ]
}

fn palette_partial_mesh_source(mode: &str, channels: usize, palette: &[u8]) -> Image {
    let source_size = (16u32, 16u32);
    let byte_count = (source_size.0 as usize)
        .checked_mul(source_size.1 as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .expect("partial palette Mesh fixture size");
    let bytes = (0..byte_count)
        .map(|index| {
            index
                .wrapping_mul(37)
                .wrapping_add(channels.wrapping_mul(11))
                .wrapping_add(5) as u8
        })
        .collect::<Vec<_>>();
    let storage_mode = if mode == "PA" { "LA" } else { mode };
    let mut source = Image::frombytes(storage_mode, source_size, &bytes)
        .unwrap_or_else(|error| panic!("{mode} source: {error}"));
    source
        .putpalette(palette, "RGB")
        .unwrap_or_else(|error| panic!("{mode} palette: {error}"));
    assert_eq!(source.mode().expect("palette mode"), mode);
    source
}

fn assert_palette_partial_mesh_mode(
    source: &Image,
    mode: &str,
    fill: &TransformFill,
    cases: &[PalettePartialMeshCase; 2],
    previous: bool,
) -> bool {
    for &(bbox, quad) in cases {
        let data = TransformData::Mesh(vec![(bbox.to_vec(), quad.to_vec())]);
        for filter in [
            (0, ResampleFilter::Nearest),
            (2, ResampleFilter::Bilinear),
            (3, ResampleFilter::Bicubic),
        ] {
            if !assert_palette_partial_mesh_case(
                source, mode, fill, bbox, quad, &data, filter, previous,
            ) {
                return false;
            }
        }
    }
    true
}

fn assert_palette_partial_mesh_case(
    source: &Image,
    mode: &str,
    fill: &TransformFill,
    bbox: [f64; 4],
    quad: [f64; 8],
    data: &TransformData,
    filter: (i32, ResampleFilter),
    previous: bool,
) -> bool {
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
    let parity_context = format!("native {mode} partial Mesh filter={filter:?}");
    let telemetry_context = format!("mode={mode} bbox={bbox:?} quad={quad:?} filter={filter:?}");
    let operation_context = format!("{mode} partial Mesh transform");
    assert_projective_gpu_parity_case(
        transformed,
        previous,
        &parity_context,
        "CPU partial palette Mesh transform",
        &operation_context,
        &telemetry_context,
    )
}
