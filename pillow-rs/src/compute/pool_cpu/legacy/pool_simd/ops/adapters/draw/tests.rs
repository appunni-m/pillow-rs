//! Focused contract tests for SIMD Draw kernels.

use crate::raster::{DynamicImage, RgbImage, RgbaImage};

fn rgba_fixture() -> DynamicImage {
    let bytes = (0..64 * 40 * 4)
        .map(|index| ((index * 31 + 7) & 255) as u8)
        .collect();
    DynamicImage::ImageRgba8(RgbaImage::from_raw(64, 40, bytes).expect("valid RGBA storage"))
}

#[test]
fn line_kernels_match_cpu_for_horizontal_diagonal_and_wide_lines() {
    let image = rgba_fixture();
    let fill = (19, 83, 147, 211);

    for (x0, y0, x1, y1, width) in [(2, 4, 60, 4, 1), (2, 3, 59, 33, 1), (5, 3, 57, 34, 4)] {
        let expected = crate::compute::pool_cpu::ops::draw::op_draw_line(
            &image, x0, y0, x1, y1, fill, width, false, None,
        )
        .expect("CPU line draw");
        let actual = super::simd_draw_line_native(&image, x0, y0, x1, y1, fill, width, false, None)
            .expect("SIMD line draw")
            .expect("line is admitted by the SIMD kernel");

        assert_eq!(actual.as_bytes(), expected.as_bytes(), "line width {width}");
    }
}

#[test]
fn rounded_rectangle_routes_match_cpu_for_rect_span_and_ellipse_geometry() {
    let image = rgba_fixture();
    let fill = Some((213, 71, 29, 255));
    let outline = Some((17, 109, 231, 255));

    for (radius, fill, outline, width) in [
        (0.0, fill, None, 1),
        (5.0, fill, outline, 2),
        (100.0, None, outline, 2),
    ] {
        let expected = crate::compute::pool_cpu::ops::draw::op_draw_rounded_rect(
            &image, 4, 3, 58, 35, radius, fill, outline, width, false, None,
        )
        .expect("CPU rounded rectangle draw");
        let actual = super::simd_draw_rounded_rect_native(
            &image, 4, 3, 58, 35, radius, fill, outline, width, false, None,
        )
        .expect("SIMD rounded rectangle draw")
        .expect("rounded rectangle is admitted by the SIMD kernel");

        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "rounded rectangle radius {radius}"
        );
    }
}

#[test]
fn rgb_line_kernel_matches_cpu_alpha_blending() {
    let bytes = (0..64 * 40 * 3)
        .map(|index| ((index * 17 + 23) & 255) as u8)
        .collect();
    let image =
        DynamicImage::ImageRgb8(RgbImage::from_raw(64, 40, bytes).expect("valid RGB storage"));
    let fill = (201, 67, 13, 127);
    let expected = crate::compute::pool_cpu::ops::draw::op_draw_line(
        &image,
        2,
        8,
        61,
        8,
        fill,
        1,
        true,
        Some("RGB"),
    )
    .expect("CPU blended line draw");
    let actual = super::simd_draw_line_native(&image, 2, 8, 61, 8, fill, 1, true, Some("RGB"))
        .expect("SIMD blended line draw")
        .expect("RGB line is admitted by the SIMD kernel");

    assert_eq!(actual.as_bytes(), expected.as_bytes());
}
