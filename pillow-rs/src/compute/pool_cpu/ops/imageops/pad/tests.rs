use super::op_pad;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

fn native_bytes<'a>(image: &'a DynamicImage, mode: &str) -> &'a [u8] {
    match (mode, image) {
        ("L", DynamicImage::ImageLuma8(image)) => image.as_raw(),
        ("LA", DynamicImage::ImageLumaA8(image)) => image.as_raw(),
        ("RGB" | "HSV", DynamicImage::ImageRgb8(image)) => image.as_raw(),
        ("RGBA" | "RGBX" | "CMYK", DynamicImage::ImageRgba8(image)) => image.as_raw(),
        _ => panic!("pad must preserve native {mode} storage"),
    }
}

fn check_native_pad(source: DynamicImage, mode: &str, fill: (u8, u8, u8, u8), expected: &[u8]) {
    let result = op_pad(
        &source,
        2,
        2,
        ResampleFilter::Nearest,
        Some(fill),
        (0.0, 1.0),
        Some(mode),
    )
    .expect("native pad");
    assert_eq!(native_bytes(&result, mode), expected);
}

#[test]
fn pad_keeps_l_la_rgb_hsv_and_rgba_channels_in_native_storage() {
    check_native_pad(
        DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![11, 22]).expect("L source")),
        "L",
        (73, 99, 111, 157),
        &[73, 73, 11, 22],
    );
    check_native_pad(
        DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(2, 1, vec![11, 12, 22, 23]).expect("LA source"),
        ),
        "LA",
        (73, 99, 111, 157),
        &[73, 157, 73, 157, 11, 12, 22, 23],
    );
    check_native_pad(
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![11, 12, 13, 22, 23, 24]).expect("RGB source"),
        ),
        "RGB",
        (73, 99, 111, 157),
        &[73, 99, 111, 73, 99, 111, 11, 12, 13, 22, 23, 24],
    );
    check_native_pad(
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(2, 1, vec![11, 12, 13, 22, 23, 24]).expect("HSV source"),
        ),
        "HSV",
        (73, 99, 111, 157),
        &[73, 99, 111, 73, 99, 111, 11, 12, 13, 22, 23, 24],
    );
    check_native_pad(
        DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![11, 12, 13, 14, 22, 23, 24, 25]).expect("RGBA source"),
        ),
        "RGBA",
        (73, 99, 111, 157),
        &[
            73, 99, 111, 157, 73, 99, 111, 157, 11, 12, 13, 14, 22, 23, 24, 25,
        ],
    );
}

#[cfg(not(feature = "parallel"))]
#[test]
fn pad_l_bicubic_full_width_contain_preserves_fill_and_resized_samples() {
    let source_bytes = (0..48).map(|value| (value * 37 % 256) as u8).collect();
    let source =
        DynamicImage::ImageLuma8(GrayImage::from_raw(8, 6, source_bytes).expect("native L source"));
    let resized =
        crate::ops::pil_resize::pil_resize(&source, 4, 3, ResampleFilter::Bicubic, Some("L"));
    let result = op_pad(
        &source,
        4,
        5,
        ResampleFilter::Bicubic,
        Some((19, 31, 47, 61)),
        (0.5, 0.5),
        Some("L"),
    )
    .expect("bicubic L pad");
    let expected = [
        &[19, 19, 19, 19][..],
        &resized.as_bytes()[..4],
        &resized.as_bytes()[4..8],
        &resized.as_bytes()[8..12],
        &[19, 19, 19, 19][..],
    ]
    .concat();
    assert_eq!(native_bytes(&result, "L"), expected);
}

#[test]
fn pad_keeps_cmyk_cmyk_samples_native() {
    check_native_pad(
        DynamicImage::ImageRgba8(
            RgbaImage::from_raw(2, 1, vec![11, 12, 13, 14, 22, 23, 24, 25]).expect("CMYK source"),
        ),
        "CMYK",
        (73, 99, 111, 157),
        &[
            73, 99, 111, 157, 73, 99, 111, 157, 11, 12, 13, 14, 22, 23, 24, 25,
        ],
    );
}

#[test]
fn pad_preserves_rgbx_source_and_fill_x_bytes() {
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41]).expect("RGBX source"),
    );
    let fill = (7, 11, 13, 17);
    let result = op_pad(
        &source,
        2,
        4,
        ResampleFilter::Nearest,
        Some(fill),
        (0.0, 0.5),
        Some("RGBX"),
    )
    .expect("native RGBX pad");

    let fill_row = [7, 11, 13, 17, 7, 11, 13, 17];
    let expected = [
        fill_row.as_slice(),
        &fill_row,
        &[23, 47, 89, 131, 29, 31, 37, 41],
        &fill_row,
    ]
    .concat();
    assert_eq!(native_bytes(&result, "RGBX"), expected);
}

#[test]
fn pad_defaults_rgbx_x_sample_to_zero() {
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41]).expect("RGBX source"),
    );
    let result = op_pad(
        &source,
        2,
        2,
        ResampleFilter::Nearest,
        None,
        (0.0, 0.5),
        Some("RGBX"),
    )
    .expect("native RGBX pad");

    assert_eq!(
        native_bytes(&result, "RGBX"),
        &[23, 47, 89, 131, 29, 31, 37, 41, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
