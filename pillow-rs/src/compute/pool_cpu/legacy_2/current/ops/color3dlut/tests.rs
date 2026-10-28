use super::op_color3dlut;
use crate::pipeline::PixelMode;
use crate::raster::{DynamicImage, ImageBuffer, Rgb, Rgba};

fn black_table() -> Vec<f64> {
    vec![0.0; 2 * 2 * 2 * 3]
}

#[test]
fn target_mode_selects_rgb_or_four_byte_output_storage() {
    let rgba_source =
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, Rgba([10, 20, 30, 77])));
    let rgb_result = op_color3dlut(
        &rgba_source,
        (2, 2, 2),
        &black_table(),
        3,
        PixelMode::RGBA,
        PixelMode::RGB,
    )
    .unwrap_or_else(|error| panic!("RGB target should be supported: {error}"));
    let DynamicImage::ImageRgb8(rgb_result) = rgb_result else {
        panic!("RGB target must use three-byte storage");
    };
    assert_eq!(rgb_result.get_pixel(0, 0).0, [0, 0, 0]);

    let rgba_result = op_color3dlut(
        &rgba_source,
        (2, 2, 2),
        &black_table(),
        3,
        PixelMode::RGBA,
        PixelMode::RGBA,
    )
    .unwrap_or_else(|error| panic!("RGBA target should be supported: {error}"));
    let DynamicImage::ImageRgba8(rgba_result) = rgba_result else {
        panic!("RGBA target must use four-byte storage");
    };
    assert_eq!(rgba_result.get_pixel(0, 0).0, [0, 0, 0, 77]);

    let cmyk_source =
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(1, 1, Rgba([10, 20, 30, 91])));
    let cmyk_result = op_color3dlut(
        &cmyk_source,
        (2, 2, 2),
        &black_table(),
        3,
        PixelMode::CMYK,
        PixelMode::CMYK,
    )
    .unwrap_or_else(|error| panic!("CMYK target should be supported: {error}"));
    let DynamicImage::ImageRgba8(cmyk_result) = cmyk_result else {
        panic!("CMYK target must retain four-byte storage");
    };
    assert_eq!(cmyk_result.get_pixel(0, 0).0, [0, 0, 0, 91]);
}

#[test]
fn rgb_source_gets_opaque_alpha_when_targeting_rgba() {
    let rgb_source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([10, 20, 30])));
    let result = op_color3dlut(
        &rgb_source,
        (2, 2, 2),
        &black_table(),
        3,
        PixelMode::RGB,
        PixelMode::RGBA,
    )
    .unwrap_or_else(|error| panic!("RGBA target should be supported: {error}"));
    let DynamicImage::ImageRgba8(result) = result else {
        panic!("RGBA target must use four-byte storage");
    };
    assert_eq!(result.get_pixel(0, 0).0, [0, 0, 0, 255]);
}
