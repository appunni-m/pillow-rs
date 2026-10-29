use super::{native_expand_channels, op_expand};
use crate::raster::{DynamicImage, RgbImage, RgbaImage};

#[test]
fn expand_hsv_keeps_three_native_bytes() {
    let image =
        DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![23, 47, 89]).expect("HSV source"));
    let expanded = op_expand(&image, 1, (7, 11, 13, 17), Some("HSV")).expect("native HSV expand");
    let DynamicImage::ImageRgb8(expanded) = expanded else {
        panic!("HSV expansion must retain three-byte physical storage");
    };
    assert_eq!(
        expanded.as_raw(),
        &[
            7, 11, 13, 7, 11, 13, 7, 11, 13, 7, 11, 13, 23, 47, 89, 7, 11, 13, 7, 11, 13, 7,
            11, 13, 7, 11, 13,
        ]
    );
}

#[test]
fn expand_cmyk_keeps_k_as_the_fourth_native_sample() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(1, 1, vec![23, 47, 89, 131]).expect("CMYK source"),
    );
    assert_eq!(native_expand_channels(&image, Some("CMYK")), Some(4));
    let mismatched = DynamicImage::ImageRgb8(
        RgbImage::from_raw(1, 1, vec![23, 47, 89]).expect("mismatched CMYK storage"),
    );
    assert_eq!(native_expand_channels(&mismatched, Some("CMYK")), None);

    let expanded =
        op_expand(&image, 1, (7, 11, 13, 17), Some("CMYK")).expect("native CMYK expand");
    let DynamicImage::ImageRgba8(expanded) = expanded else {
        panic!("CMYK expansion must retain four-byte storage");
    };
    assert_eq!(
        expanded.as_raw(),
        &[
            7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 23, 47, 89, 131, 7,
            11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17, 7, 11, 13, 17,
        ]
    );
}

#[test]
fn expand_rgbx_keeps_x_bytes_and_uses_fill_x_for_the_border() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41]).expect("RGBX source"),
    );
    assert_eq!(native_expand_channels(&image, Some("RGBX")), Some(4));
    let mismatched = DynamicImage::ImageRgb8(
        RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37]).expect("mismatched RGBX storage"),
    );
    assert_eq!(native_expand_channels(&mismatched, Some("RGBX")), None);

    let expanded =
        op_expand(&image, 1, (7, 11, 13, 17), Some("RGBX")).expect("native RGBX expand");
    let DynamicImage::ImageRgba8(expanded) = expanded else {
        panic!("RGBX expansion must retain four-byte storage");
    };
    let fill = [7, 11, 13, 17];
    let mut expected = Vec::with_capacity(4 * 3 * 4);
    expected.extend_from_slice(&fill.repeat(4));
    expected.extend_from_slice(&fill);
    expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
    expected.extend_from_slice(&fill);
    expected.extend_from_slice(&fill.repeat(4));
    assert_eq!(expanded.as_raw(), &expected);
}
