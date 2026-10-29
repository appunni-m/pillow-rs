use super::kernels::simd_projective_nearest_transform_bytes;
use crate::raster::{DynamicImage, GrayImage};

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

    let result = simd_projective_nearest_transform_bytes(
        &image,
        8,
        7,
        &data,
        Some((17, 0, 0, 255)),
        Some("L"),
        false,
    )
    .expect("SIMD perspective transform")
    .expect("SIMD luma layout");
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

    let result = simd_projective_nearest_transform_bytes(
        &image,
        8,
        7,
        &data,
        Some((17, 0, 0, 255)),
        Some("L"),
        true,
    )
    .expect("SIMD quad transform")
    .expect("SIMD luma layout");
    let expected = [
        3, 14, 25, 36, 47, 58, 69, 80, 40, 51, 62, 73, 84, 95, 106, 117, 114, 125, 136, 147, 158,
        169, 180, 191, 151, 162, 173, 184, 195, 206, 217, 228, 188, 199, 210, 221, 232, 243, 254,
        9, 225, 236, 247, 2, 13, 61, 72, 83, 43, 54, 65, 76, 87, 98, 109, 120,
    ];

    assert_eq!(result.as_bytes(), expected);
}
