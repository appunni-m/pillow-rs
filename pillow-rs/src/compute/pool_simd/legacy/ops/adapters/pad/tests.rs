//! Focused contract tests for the SIMD Pad family.

#[test]
fn pad_rgbx_preserves_x_samples_and_fills_new_pixels_from_fill_x() {
    use crate::raster::DynamicImage;

    let image = DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
            .expect("RGBX source shape must be valid"),
    );
    assert_eq!(
        super::super::resize::native_pad_channels_for_image(&image, Some("RGBX")),
        Some(4)
    );
    let mismatched = DynamicImage::ImageRgb8(
        crate::raster::RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37])
            .expect("mismatched RGBX storage shape must be valid"),
    );
    assert_eq!(
        super::super::resize::native_pad_channels_for_image(&mismatched, Some("RGBX")),
        None
    );

    let fill = (7, 11, 13, 17);
    let operation = crate::pipeline::PipelineOp::Pad {
        w: 2,
        h: 4,
        filter: crate::pipeline::ResampleFilter::Nearest,
        color: Some(fill),
        centering: (0.0, 0.5),
    };
    let result =
        super::simd_pad(&image, &operation, Some("RGBX")).expect("SIMD RGBX pad must succeed");

    let fill_row = [7, 11, 13, 17, 7, 11, 13, 17];
    let mut expected = Vec::with_capacity(2 * 4 * 4);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
    expected.extend_from_slice(&fill_row);
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn pad_rgba_appends_native_source_rows_between_alpha_fill_rows() {
    use crate::raster::{DynamicImage, RgbaImage};

    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
            .expect("RGBA source shape must be valid"),
    );
    let fill = (7, 11, 13, 17);
    let operation = crate::pipeline::PipelineOp::Pad {
        w: 2,
        h: 4,
        filter: crate::pipeline::ResampleFilter::Nearest,
        color: Some(fill),
        centering: (0.0, 0.5),
    };
    let result = super::simd_pad(&source, &operation, Some("RGBA")).expect("SIMD RGBA pad");

    let fill_row = [7, 11, 13, 17, 7, 11, 13, 17];
    let mut expected = Vec::with_capacity(2 * 4 * 4);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
    expected.extend_from_slice(&fill_row);
    assert_eq!(result.as_bytes(), expected);
}

#[test]
fn pad_rgb_appends_native_source_rows_between_fill_rows() {
    use crate::raster::{DynamicImage, RgbImage};

    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(2, 1, vec![23, 47, 89, 29, 31, 37])
            .expect("RGB source shape must be valid"),
    );
    let fill = (7, 11, 13, 17);
    let operation = crate::pipeline::PipelineOp::Pad {
        w: 2,
        h: 4,
        filter: crate::pipeline::ResampleFilter::Nearest,
        color: Some(fill),
        centering: (0.0, 0.5),
    };
    let result = super::simd_pad(&source, &operation, Some("RGB")).expect("SIMD RGB pad");

    let fill_row = [7, 11, 13, 7, 11, 13];
    let mut expected = Vec::with_capacity(2 * 4 * 3);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&fill_row);
    expected.extend_from_slice(&[23, 47, 89, 29, 31, 37]);
    expected.extend_from_slice(&fill_row);
    assert_eq!(result.as_bytes(), expected);
}
