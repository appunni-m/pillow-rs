use super::projective::cubic_sample;
use super::{op_transform, transform_mesh, transform_projective_generic};
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};

fn varied_luma_source() -> DynamicImage {
    let raw: Vec<u8> = (0..4)
        .flat_map(|y| (0..5).map(move |x| (x * 37 + y * 11 + 3) as u8))
        .collect();
    DynamicImage::ImageLuma8(GrayImage::from_raw(5, 4, raw).expect("luma source"))
}

fn varied_luma_alpha_source() -> DynamicImage {
    let raw: Vec<u8> = (0..4)
        .flat_map(|y| {
            (0..5).flat_map(move |x| {
                let luma = (x * 37 + y * 11 + 3) as u8;
                [luma, luma.wrapping_add(83)]
            })
        })
        .collect();
    DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(5, 4, raw).expect("LA source"))
}

#[test]
fn affine_l_nearest_direct_path_matches_generic_and_is_selected() {
    let source = varied_luma_source();
    let affine = [0.87, 0.21, -1.25, -0.16, 1.13, 0.5];
    let fill = Some((173, 0, 0, 255));
    let specialized = super::transform_affine_luma_nearest(&source, 7, 5, affine, fill)
        .expect("L nearest specialization")
        .expect("L storage and fixed affine plan should use the direct path");
    let generic = super::transform_affine_generic(
        &source, 7, 5, affine[0], affine[1], affine[2], affine[3], affine[4], affine[5], fill,
        true, false,
    )
    .expect("generic L nearest transform");

    assert_eq!(specialized, generic);
    for explicit_mode in [None, Some("L")] {
        let actual = op_transform(
            &source,
            7,
            5,
            &TransformMethod::Affine,
            &affine,
            &ResampleFilter::Nearest,
            fill,
            false,
            explicit_mode,
        )
        .expect("public L affine nearest transform");
        assert_eq!(actual, generic, "mode {explicit_mode:?}");
    }
}

#[test]
fn affine_la_nearest_direct_path_matches_generic_and_is_selected() {
    let source = varied_luma_alpha_source();
    let affine = [0.87, 0.21, -1.25, -0.16, 1.13, 0.5];
    let fill = Some((173, 127, 0, 0));
    let specialized = super::transform_affine_luma_alpha_nearest(&source, 7, 5, affine, fill)
        .expect("LA nearest specialization")
        .expect("LA storage and fixed affine plan should use the direct path");
    let generic = super::transform_affine_generic(
        &source, 7, 5, affine[0], affine[1], affine[2], affine[3], affine[4], affine[5], fill,
        true, false,
    )
    .expect("generic LA nearest transform");

    assert_eq!(specialized, generic);
    for explicit_mode in [None, Some("LA")] {
        let actual = op_transform(
            &source,
            7,
            5,
            &TransformMethod::Affine,
            &affine,
            &ResampleFilter::Nearest,
            fill,
            false,
            explicit_mode,
        )
        .expect("public LA affine nearest transform");
        assert_eq!(actual, generic, "mode {explicit_mode:?}");
    }
}

#[test]
fn affine_rgb_nearest_direct_path_matches_generic_and_is_selected() {
    let source = varied_rgb_source();
    let affine = [0.87, 0.21, -1.25, -0.16, 1.13, 0.5];
    let fill = Some((173, 91, 57, 255));
    let specialized = super::transform_affine_rgb_nearest(&source, 7, 5, affine, fill)
        .expect("RGB nearest specialization")
        .expect("RGB storage and fixed affine plan should use the direct path");
    let generic = super::transform_affine_generic(
        &source, 7, 5, affine[0], affine[1], affine[2], affine[3], affine[4], affine[5], fill,
        true, false,
    )
    .expect("generic RGB nearest transform");

    assert_eq!(specialized, generic);
    for explicit_mode in [None, Some("RGB"), Some("HSV"), Some("YCbCr")] {
        let actual = op_transform(
            &source,
            7,
            5,
            &TransformMethod::Affine,
            &affine,
            &ResampleFilter::Nearest,
            fill,
            false,
            explicit_mode,
        )
        .expect("public RGB-family affine nearest transform");
        assert_eq!(actual, generic, "mode {explicit_mode:?}");
    }
}

fn varied_rgb_source() -> DynamicImage {
    let raw: Vec<u8> = (0..4)
        .flat_map(|y| {
            (0..5).flat_map(move |x| {
                let value = (x * 37 + y * 11 + 3) as u8;
                [value, value.wrapping_add(37), value.wrapping_add(74)]
            })
        })
        .collect();
    DynamicImage::ImageRgb8(RgbImage::from_raw(5, 4, raw).expect("rgb source"))
}

fn varied_rgba_source() -> DynamicImage {
    let raw: Vec<u8> = (0..8)
        .flat_map(|y| {
            (0..9).flat_map(move |x| {
                let index = (y * 9 + x) * 4;
                [
                    (index * 37 + (index / 4) * 11 + 3) as u8,
                    ((index + 1) * 37 + ((index + 1) / 4) * 11 + 3) as u8,
                    ((index + 2) * 37 + ((index + 2) / 4) * 11 + 3) as u8,
                    ((index + 3) * 37 + ((index + 3) / 4) * 11 + 3) as u8,
                ]
            })
        })
        .collect();
    DynamicImage::ImageRgba8(RgbaImage::from_raw(9, 8, raw).expect("rgba source"))
}

fn f32_source(values: &[f32], width: u32, height: u32) -> DynamicImage {
    let raw = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, raw).expect("F source buffer shape"),
    )
}

#[cfg(target_endian = "little")]
#[test]
fn f_projective_filters_use_scalar_words() {
    let source = f32_source(&[1.25, -2.5, 7.75, 10.5], 2, 2);
    for (method, data, filter, expected) in f_projective_filter_cases() {
        let output = op_transform(
            &source,
            3,
            3,
            &method,
            &data,
            &filter,
            Some((0, 0, 0, 0)),
            false,
            Some("F"),
        )
        .expect("F projective transform");
        let words = output
            .as_bytes()
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect::<Vec<_>>();
        assert_eq!(words, expected);
    }
}

type FProjectiveFilterCase = (TransformMethod, Vec<f64>, ResampleFilter, [u32; 9]);

#[cfg(target_endian = "little")]
fn f_projective_filter_cases() -> [FProjectiveFilterCase; 3] {
    [
        (
            TransformMethod::Perspective,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            ResampleFilter::Bilinear,
            [
                0x3fa0_0000,
                0xc020_0000,
                0x0000_0000,
                0x40f8_0000,
                0x4128_0000,
                0x0000_0000,
                0x0000_0000,
                0x0000_0000,
                0x0000_0000,
            ],
        ),
        (
            TransformMethod::Quad,
            vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0],
            ResampleFilter::Bicubic,
            [
                0x3f82_6798,
                0x4091_da13,
                0x4101_8d20,
                0xbfe0_71c7,
                0x4088_0000,
                0x4124_0e39,
                0xc090_d2ca,
                0x407c_4bda,
                0x4146_8f52,
            ],
        ),
        (
            TransformMethod::Mesh,
            vec![0.0, 0.0, 3.0, 3.0, 0.0, 0.0, 2.0, 0.0, 2.0, 2.0, 0.0, 2.0],
            ResampleFilter::Bilinear,
            [
                0x3fa0_0000,
                0x4090_0000,
                0x40f8_0000,
                0xbf20_0000,
                0x4088_0000,
                0x4112_0000,
                0xc020_0000,
                0x4080_0000,
                0x4128_0000,
            ],
        ),
    ]
}

#[cfg(target_endian = "little")]
#[test]
fn f_mesh_fill_presence_matches_generic_transform() {
    let source = f32_source(&[5.0], 1, 1);
    let mesh = [
        0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, -1.0, -1.0,
        -1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
    ];
    let fill = Some((0x00, 0x00, 0xe0, 0x40));

    let explicit = op_transform(
        &source,
        1,
        1,
        &TransformMethod::Mesh,
        &mesh,
        &ResampleFilter::Bilinear,
        fill,
        false,
        Some("F"),
    )
    .expect("explicit F mesh fill");
    let omitted = op_transform(
        &source,
        1,
        1,
        &TransformMethod::Mesh,
        &mesh,
        &ResampleFilter::Bilinear,
        fill,
        true,
        Some("F"),
    )
    .expect("omitted F mesh fill");

    let word = |image: &DynamicImage| {
        u32::from_le_bytes([
            image.as_bytes()[0],
            image.as_bytes()[1],
            image.as_bytes()[2],
            image.as_bytes()[3],
        ])
    };
    assert_eq!(word(&explicit), 5.0f32.to_bits());
    assert_eq!(word(&omitted), 0.0f32.to_bits());
}

const VARIED_MESH: [f64; 12] = [
    -1.0, -1.0, 6.0, 5.0, -0.4, 0.2, 4.6, 3.4, 4.1, -0.3, 0.3, 3.7,
];

#[test]
fn mesh_nearest_matches_pillow_centers_and_truncation() {
    let result = transform_mesh(
        &varied_luma_source(),
        6,
        5,
        &VARIED_MESH,
        Some((7, 0, 0, 0)),
        ResampleFilter::Nearest,
        false,
    )
    .expect("mesh transform");
    let expected = [
        3, 14, 14, 14, 25, 25, 14, 14, 14, 51, 62, 62, 51, 51, 51, 51, 51, 51, 99, 88, 88, 88, 88,
        88, 136, 136, 125, 125, 125, 125,
    ];
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn mesh_bilinear_matches_pillow_horizontal_first_filter() {
    let result = transform_mesh(
        &varied_rgb_source(),
        6,
        5,
        &VARIED_MESH,
        Some((7, 9, 11, 13)),
        ResampleFilter::Bilinear,
        false,
    )
    .expect("mesh transform");
    let expected = [
        4, 41, 78, 9, 46, 83, 14, 51, 88, 18, 55, 92, 23, 60, 97, 27, 64, 101, 23, 60, 97, 28, 65,
        102, 33, 70, 107, 38, 75, 112, 42, 79, 116, 47, 84, 121, 59, 96, 133, 60, 97, 134, 62, 99,
        136, 64, 101, 138, 66, 103, 140, 68, 105, 142, 94, 131, 168, 93, 130, 167, 92, 129, 166,
        90, 127, 164, 89, 126, 163, 88, 125, 162, 129, 166, 203, 125, 162, 199, 121, 158, 195, 117,
        154, 191, 113, 150, 187, 109, 146, 183,
    ];
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn mesh_bilinear_preserves_native_quad_fma_order() {
    let image = DynamicImage::ImageLuma8(
        GrayImage::from_raw(3, 2, vec![1, 5, 13, 14, 28, 45]).expect("luma source"),
    );
    let mesh = [
        1.0, -5.0, 5.0, 6.0, -0.25, -2.125, 3.375, 4.375, -0.875, -2.875, 10.5, 3.125,
    ];

    let result = transform_mesh(
        &image,
        8,
        7,
        &mesh,
        Some((0, 0, 0, 255)),
        ResampleFilter::Bilinear,
        false,
    )
    .expect("mesh transform");

    // At (2, 5), Pillow's compiled quad_transform maps to y=0.75 and
    // bilinear filtering returns 21. Keeping the cross-term multiply
    // separate from the final fused y-product preserves that boundary.
    assert_eq!(result.as_bytes()[5 * 8 + 2], 21);
}

#[test]
fn mesh_rgba_bilinear_matches_pillow_alpha_roundtrip() {
    let result = op_transform(
        &varied_rgba_source(),
        9,
        8,
        &TransformMethod::Mesh,
        &[2.0, 0.0, 9.0, 7.0, 7.1, -0.5, 1.2, 3.7, 4.8, 8.3, -1.2, 6.9],
        &ResampleFilter::Bilinear,
        Some((7, 9, 11, 13)),
        false,
        Some("RGBA"),
    )
    .expect("mesh transform");
    assert!(matches!(result, DynamicImage::ImageRgba8(_)));
    assert_eq!(result.dimensions(), (9, 8));
    let bytes = result.as_bytes();
    assert_eq!(&bytes[0..4], &[137, 176, 215, 13]);
    assert_eq!(
        &bytes[(3 * 9 + 4) * 4..(3 * 9 + 5) * 4],
        &[64, 101, 141, 63]
    );
    assert_eq!(&bytes[(5 * 9 + 8) * 4..(5 * 9 + 9) * 4], &[8, 47, 85, 117]);
    assert_eq!(
        &bytes[(6 * 9 + 7) * 4..(6 * 9 + 8) * 4],
        &[49, 87, 124, 119]
    );
}

#[test]
fn mesh_bicubic_matches_pillow_clamped_filter() {
    let result = transform_mesh(
        &varied_luma_source(),
        6,
        5,
        &VARIED_MESH,
        Some((7, 0, 0, 0)),
        ResampleFilter::Bicubic,
        false,
    )
    .expect("mesh transform");
    let expected = [
        0, 2, 8, 14, 19, 28, 18, 23, 29, 33, 37, 41, 63, 65, 67, 68, 70, 71, 94, 94, 93, 92, 91,
        89, 125, 121, 118, 114, 109, 104,
    ];
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn cubic_sample_preserves_native_horner_fma_order() {
    // This tap sequence occurs at the top edge of a one-column RGBX
    // source. Pillow src/libImaging/Geometry.c BICUBIC uses fused Horner
    // steps and returns 37; separate multiply/add operations return 38.
    let value = cubic_sample([174.0, 36.0, 38.0, 84.0], 0.9999999999999996);
    assert_eq!(value as u8, 37);
}

#[test]
fn perspective_nearest_matches_pillow_center_and_truncation() {
    let width = 9;
    let height = 8;
    let raw: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
        .collect();
    let image =
        DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
    let data = [1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002];

    let result = transform_projective_generic(
        &image,
        8,
        7,
        &data,
        &ResampleFilter::Nearest,
        Some((17, 0, 0, 255)),
        false,
    )
    .expect("perspective transform");
    let expected = [
        3, 40, 77, 114, 151, 188, 225, 6, 51, 88, 125, 162, 162, 199, 236, 17, 62, 99, 136, 173,
        210, 247, 28, 65, 73, 110, 147, 184, 221, 2, 39, 76, 84, 121, 158, 195, 232, 13, 50, 87,
        95, 132, 169, 206, 243, 24, 61, 98, 106, 143, 180, 217, 254, 35, 72, 109,
    ];

    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn quad_nearest_matches_pillow_center_and_truncation() {
    let width = 9;
    let height = 8;
    let raw: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
        .collect();
    let image =
        DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
    // Pillow's QUAD input order is NW, SW, SE, NE.
    let data = [0.2, 0.1, 8.7, 0.2, 9.1, 7.8, -0.4, 8.3];

    let result = transform_projective_generic(
        &image,
        8,
        7,
        &data,
        &ResampleFilter::Nearest,
        Some((17, 0, 0, 255)),
        true,
    )
    .expect("quad transform");
    let expected = [
        3, 14, 25, 36, 47, 58, 69, 80, 40, 51, 62, 73, 84, 95, 106, 117, 114, 125, 136, 147, 158,
        169, 180, 191, 151, 162, 173, 184, 195, 206, 217, 228, 188, 199, 210, 221, 232, 243, 254,
        9, 225, 236, 247, 2, 13, 61, 72, 83, 43, 54, 65, 76, 87, 98, 109, 120,
    ];

    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn f_affine_filters_interpolate_float_words() {
    let source = f32_source(&[1.0, 2.0, 3.0, 4.0], 2, 2);
    let matrix = [1.0, 0.5, 0.0, 0.0, 1.0, 0.0];
    let expected_bilinear = [
        0x3fa0_0000,
        0x4000_0000,
        0x0000_0000,
        0x4070_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
    ];
    let expected_bicubic = [
        0x3fa0_0000,
        0x4009_0000,
        0x0000_0000,
        0x4070_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
    ];

    for (filter, expected) in [
        (ResampleFilter::Bilinear, expected_bilinear),
        (ResampleFilter::Bicubic, expected_bicubic),
    ] {
        let result = op_transform(
            &source,
            3,
            3,
            &TransformMethod::Affine,
            &matrix,
            &filter,
            Some((0, 0, 0, 0)),
            false,
            Some("F"),
        )
        .expect("F affine filter");
        let words = result
            .as_bytes()
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect::<Vec<_>>();
        assert_eq!(words, expected);
    }
}

#[test]
fn f_affine_coordinate_fma_preserves_float32_boundary() {
    // At destination (1, 0), Geometry.c's compiled affine callback maps
    // this matrix to exactly (1.5, 0.75).  Computing the two products
    // separately produces 1.5000000000000002 and lets a huge neighbouring
    // FLOAT32 word contaminate the bilinear result.
    let source = f32_source(
        &[
            f32::from_bits(0x6bc6_6dcc),
            f32::from_bits(0xac52_ded2),
            f32::from_bits(0xf587_a6b6),
            f32::from_bits(0x984a_6295),
            f32::from_bits(0x2e79_39ae),
            f32::from_bits(0x2113_8c85),
        ],
        3,
        2,
    );
    let result = op_transform(
        &source,
        7,
        5,
        &TransformMethod::Affine,
        &[0.8, 0.2, 0.2, -0.3, 2.0, 0.2],
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 0)),
        false,
        Some("F"),
    )
    .expect("F affine boundary transform");
    let words = result
        .as_bytes()
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect::<Vec<_>>();
    assert_eq!(words[1], 0x2d51_afe7);
}

#[test]
fn projective_bilinear_matches_pillow_center_and_edge_sampling() {
    let width = 9;
    let height = 8;
    let raw: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x * 37 + y * 11 + 3) as u8))
        .collect();
    let image =
        DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, raw).expect("luma source"));
    let cases = [
        (
            false,
            [1.0, 0.07, 0.4, -0.03, 1.0, 0.2, 0.001, -0.002],
            [
                21, 57, 94, 130, 167, 203, 138, 20, 34, 71, 108, 144, 181, 217, 130, 34, 48, 85,
                122, 158, 195, 226, 123, 48, 62, 99, 135, 172, 209, 78, 26, 62, 76, 113, 149, 186,
                223, 71, 40, 76, 90, 127, 163, 200, 237, 64, 54, 91, 104, 141, 178, 214, 223, 55,
                68, 105,
            ],
        ),
        (
            true,
            [0.2, 0.1, 8.7, 0.2, 9.1, 7.8, -0.4, 8.3],
            [
                14, 23, 31, 40, 49, 60, 71, 80, 59, 69, 78, 87, 97, 106, 115, 122, 105, 115, 124,
                134, 144, 154, 164, 172, 150, 160, 171, 181, 192, 202, 212, 222, 195, 206, 212,
                181, 191, 200, 207, 18, 141, 146, 138, 19, 31, 42, 54, 65, 30, 42, 54, 66, 78, 90,
                102, 114,
            ],
        ),
    ];

    for (quad, data, expected) in cases {
        let result = transform_projective_generic(
            &image,
            8,
            7,
            &data,
            &ResampleFilter::Bilinear,
            Some((17, 0, 0, 255)),
            quad,
        )
        .expect("bilinear projective transform");
        assert_eq!(result.as_bytes(), expected, "quad={quad}");
    }
}

#[test]
fn projective_filtered_alpha_roundtrip_matches_pillow() {
    let data = [0.8, 0.15, -0.4, -0.12, 1.1, 0.2, 0.0, 0.0];
    let la_raw = (0..40)
        .map(|index| ((index * 53 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let la = DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(5, 4, la_raw).expect("LA source"));
    let rgba_raw = (0..80)
        .map(|index| ((index * 53 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(5, 4, rgba_raw).expect("RGBA source"));
    assert_la_perspective_alpha_roundtrip(&la, &data);
    assert_rgba_perspective_alpha_roundtrip(&rgba, &data);
    assert_rgbx_affine_filtered_storage(&rgba, &data);
    assert_rgba_quad_alpha_roundtrip(&rgba, &data);
}

fn assert_la_perspective_alpha_roundtrip(la: &DynamicImage, data: &[f64; 8]) {
    // Pillow converts LA to La before filtered projective sampling and
    // converts back after the operation.
    let perspective = op_transform(
        la,
        6,
        5,
        &TransformMethod::Perspective,
        data,
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 255)),
        false,
        Some("LA"),
    )
    .expect("LA perspective transform");
    assert_eq!(&perspective.as_bytes()[..2], &[20, 73]);
    assert_eq!(&perspective.as_bytes()[40..42], &[120, 136]);
}

fn assert_rgba_perspective_alpha_roundtrip(rgba: &DynamicImage, data: &[f64; 8]) {
    // The fused coordinate grouping matches Geometry.c. At the bottom
    // edge, unfused arithmetic lands one ULP lower and changes red.
    let perspective = op_transform(
        rgba,
        6,
        5,
        &TransformMethod::Perspective,
        data,
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 255)),
        false,
        Some("RGBA"),
    )
    .expect("RGBA perspective transform");
    assert_eq!(&perspective.as_bytes()[..4], &[25, 77, 130, 182]);
    assert_eq!(&perspective.as_bytes()[80..84], &[54, 108, 161, 212]);
}

fn assert_rgbx_affine_filtered_storage(rgba: &DynamicImage, data: &[f64; 8]) {
    // RGBX's fourth byte is padding, not alpha, so all four stored bytes
    // use the direct filtered path.
    let affine_rgba = op_transform(
        rgba,
        6,
        5,
        &TransformMethod::Affine,
        &data[..6],
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 255)),
        false,
        Some("RGBA"),
    )
    .expect("RGBA affine transform");
    assert_eq!(&affine_rgba.as_bytes()[..4], &[25, 77, 130, 182]);
    assert_eq!(&affine_rgba.as_bytes()[80..84], &[54, 108, 161, 212]);

    let affine = op_transform(
        rgba,
        6,
        5,
        &TransformMethod::Affine,
        &data[..6],
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 255)),
        false,
        Some("RGBX"),
    )
    .expect("RGBX affine transform");
    assert_eq!(&affine.as_bytes()[..4], &[23, 76, 129, 182]);
    assert_eq!(&affine.as_bytes()[40..44], &[173, 210, 68, 76]);

    let affine_bicubic = op_transform(
        rgba,
        6,
        5,
        &TransformMethod::Affine,
        &data[..6],
        &ResampleFilter::Bicubic,
        Some((0, 0, 0, 255)),
        false,
        Some("RGBX"),
    )
    .expect("RGBX affine bicubic transform");
    assert_eq!(&affine_bicubic.as_bytes()[..4], &[0, 81, 134, 187]);
    assert_eq!(&affine_bicubic.as_bytes()[40..44], &[175, 240, 51, 74]);
}

fn assert_rgba_quad_alpha_roundtrip(rgba: &DynamicImage, data: &[f64; 8]) {
    let quad = op_transform(
        rgba,
        6,
        5,
        &TransformMethod::Quad,
        data,
        &ResampleFilter::Bilinear,
        Some((0, 0, 0, 255)),
        false,
        Some("RGBA"),
    )
    .expect("RGBA quad transform");
    assert_eq!(&quad.as_bytes()[..4], &[39, 64, 118, 170]);
}

#[test]
fn projective_bicubic_matches_raw_palette_alpha_filter() {
    // Pillow's PA projective path keeps the index/alpha bands raw.  Before
    // this branch, every non-nearest projective request used the bilinear
    // kernel, so the first output pair for a fractional map was (7, 18)
    // instead of Geometry.c's bicubic (6, 16).
    let source = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(4, 1, vec![3, 11, 20, 40, 37, 69, 54, 98]).expect("PA source"),
    );
    let result = transform_projective_generic(
        &source,
        4,
        1,
        &[1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0],
        &ResampleFilter::Bicubic,
        Some((61, 233, 0, 0)),
        false,
    )
    .expect("PA bicubic projective transform");
    assert_eq!(result.as_bytes(), &[6, 16, 25, 49, 43, 80, 56, 102]);
}
