//! Focused contract tests for the SIMD median and rank filters.

#[test]
fn simd_native_l_rank_one_matches_cpu_for_ties_edges_and_vector_tails() {
    use crate::compute::registry;
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayImage};

    let operation = PipelineOp::RankFilter { size: 3, rank: 1 };
    for (width, height) in [(17u32, 1u32), (31, 9), (33, 35)] {
        let raw = (0..width as usize * height as usize)
            .map(|index| ((index * 37 + index / width as usize * 11 + 3) % 5) as u8)
            .collect::<Vec<_>>();
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, raw).expect("valid native L image"),
        );
        let expected = registry::execute_cpu(&operation, &image, Some("L"))
            .expect("CPU L RankFilter reference");
        let actual = super::simd_order_statistic_filter(&image, 3, 1, Some("L"))
            .expect("SIMD native L RankFilter");

        assert!(matches!(actual, DynamicImage::ImageLuma8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
    }
}
