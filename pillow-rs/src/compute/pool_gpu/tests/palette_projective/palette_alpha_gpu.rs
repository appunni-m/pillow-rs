use crate::ops::imageops::ImageOpsColor;
use crate::ops::rotate::{RotateExpandInput, RotatePointInput, RotateResampleInput};
use crate::ops::transform::{TransformData, TransformFill};
use crate::pipeline::ResampleFilter;
use crate::{Backend, Image};

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

    let cases = palette_alpha_nearest_cases();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    if !run_palette_alpha_nearest_cases(&source, cases) {
        Backend::set_pipeline_telemetry_enabled(previous);
        return;
    }
    let constant_cases = palette_alpha_constant_cases();
    if !run_palette_alpha_constant_cases(&source, constant_cases) {
        Backend::set_pipeline_telemetry_enabled(previous);
        return;
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

type PaletteAlphaNearestCase = ((u32, u32), [f64; 8], Option<TransformFill>);
type PaletteAlphaConstantCase = ((u32, u32), i32, TransformData, Option<TransformFill>);

fn palette_alpha_nearest_cases() -> [PaletteAlphaNearestCase; 10] {
    [
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
    ]
}

fn run_palette_alpha_nearest_cases(source: &Image, cases: [PaletteAlphaNearestCase; 10]) -> bool {
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
                return false;
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
    true
}

fn palette_alpha_constant_cases() -> [PaletteAlphaConstantCase; 6] {
    [
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
    ]
}

fn run_palette_alpha_constant_cases(
    source: &Image,
    constant_cases: [PaletteAlphaConstantCase; 6],
) -> bool {
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
                return false;
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
    true
}

#[test]
fn palette_alpha_projective_filtered_relocations_native_gpu_preserve_pairs() {
    let cases = palette_alpha_filtered_relocation_cases();
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
        if !assert_palette_filtered_case(&source, source_size, output_size, matrix, fill, &palette)
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

type PaletteAlphaFilteredCase = ((u32, u32), (u32, u32), [f64; 8], Option<TransformFill>);

fn palette_alpha_filtered_relocation_cases() -> [PaletteAlphaFilteredCase; 8] {
    [
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
    ]
}

fn assert_palette_filtered_case(
    source: &Image,
    source_size: (u32, u32),
    output_size: (u32, u32),
    matrix: [f64; 8],
    fill: Option<TransformFill>,
    palette: &[u8],
) -> bool {
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
        assert_eq!(transformed.palette(), Some(palette.to_vec()));
        let expected_image = transformed.clone().use_backend(Backend::Cpu);
        let expected = expected_image
            .tobytes()
            .expect("CPU PA filtered projective transform");
        let gpu_image = transformed.clone().use_backend(Backend::Gpu);
        assert_eq!(gpu_image.mode().expect("GPU PA mode"), "PA");
        assert_eq!(gpu_image.palette(), Some(palette.to_vec()));
        let actual = match gpu_image.tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                return false;
            }
            Err(error) => panic!("native GPU PA filtered projective transform failed: {error}"),
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
    true
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
