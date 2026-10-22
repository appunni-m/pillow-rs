//! Focused contract tests for the SIMD Expand family.

#[test]
fn expand_rgbx_appends_rows_without_changing_x_samples() {
    use crate::pipeline::PipelineOp;
    use crate::raster::DynamicImage;

    let source = DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(2, 1, vec![23, 47, 89, 131, 29, 31, 37, 41])
            .expect("RGBX source shape must be valid"),
    );
    let fill = (7, 11, 13, 17);
    let operation = PipelineOp::Expand { border: 1, fill };
    let actual = super::simd_expand(&source, &operation, Some("RGBX"))
        .expect("SIMD RGBX expansion must succeed");
    let expected = crate::compute::registry::execute_cpu(&operation, &source, Some("RGBX"))
        .expect("CPU RGBX expansion must succeed");

    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let fill_pixel = [7, 11, 13, 17];
    let mut expected_bytes = Vec::with_capacity(4 * 3 * 4);
    expected_bytes.extend_from_slice(&fill_pixel.repeat(4));
    expected_bytes.extend_from_slice(&fill_pixel);
    expected_bytes.extend_from_slice(&[23, 47, 89, 131, 29, 31, 37, 41]);
    expected_bytes.extend_from_slice(&fill_pixel);
    expected_bytes.extend_from_slice(&fill_pixel.repeat(4));
    assert_eq!(actual.as_bytes(), expected_bytes);
}

#[test]
fn expand_single_byte_modes_append_vector_filled_rows_without_zeroing() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage};

    for (width, height, border) in [(1u32, 1u32, 1u32), (17, 3, 2), (33, 32, 1), (16, 16, 0)] {
        let pixels = (0..width * height)
            .map(|index| (index.wrapping_mul(67).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, pixels).expect("L source shape must be valid"),
        );
        let operation = PipelineOp::Expand {
            border,
            fill: (7, 11, 13, 17),
        };
        for mode in [Some("L"), Some("P")] {
            let actual = super::simd_expand(&source, &operation, mode)
                .expect("SIMD single-byte expansion must succeed");
            let expected = crate::compute::registry::execute_cpu(&operation, &source, mode)
                .expect("CPU single-byte expansion must succeed");

            assert_eq!(
                (actual.width(), actual.height()),
                (expected.width(), expected.height())
            );
            assert_eq!(actual.as_bytes(), expected.as_bytes());
        }
    }
}
