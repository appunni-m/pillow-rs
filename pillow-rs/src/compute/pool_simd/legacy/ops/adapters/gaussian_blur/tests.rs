//! Focused contract tests for the SIMD GaussianBlur family.

#[test]
fn gaussian_blur_l_sliding_vertical_matches_cpu_edges_and_vector_tails() {
    use crate::compute::pool_cpu::ops::filter::execute_gaussian_blur;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage};

    for (width, height) in [(1u32, 1u32), (15, 2), (16, 3), (17, 5), (65, 47)] {
        let source = (0..width as usize * height as usize)
            .map(|index| (index.wrapping_mul(67).wrapping_add(index / 7 * 23 + 17) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, source).expect("L blur source shape"),
        );
        let expected = execute_gaussian_blur(&image, 2.0).expect("CPU L GaussianBlur");
        let actual = super::simd_pil_gaussian_blur_l_radius_one(&image, 1.375, Some("L"))
            .expect("SIMD L radius-one GaussianBlur");
        let operation = PipelineOp::GaussianBlur { sigma: 2.0 };
        let dispatched = super::simd_gaussian_blur(&image, &operation, Some("L"))
            .expect("SIMD L GaussianBlur adapter");

        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "size={width}x{height}"
        );
        assert_eq!(
            dispatched.as_bytes(),
            expected.as_bytes(),
            "adapter size={width}x{height}"
        );
    }
}
