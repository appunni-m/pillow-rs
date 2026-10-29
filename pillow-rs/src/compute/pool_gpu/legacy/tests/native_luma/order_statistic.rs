use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayImage, RgbImage};

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_order_statistic_admits_only_exact_native_l_singletons() {
    let median = PipelineOp::MedianFilter { size: 3 };
    let max_filter = PipelineOp::MaxFilter { size: 3 };
    let min_filter = PipelineOp::MinFilter { size: 3 };
    let rank = PipelineOp::RankFilter { size: 9, rank: 40 };
    let rank_one = PipelineOp::RankFilter { size: 3, rank: 1 };
    let luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(3, 2, vec![5, 40, 250, 100, 80, 10]).unwrap());
    assert_gpu_packed_luma_order_statistic_accepts(
        &luma,
        &median,
        &rank,
        &rank_one,
        &max_filter,
        &min_filter,
    );
    assert_gpu_packed_luma_order_statistic_rejects_mode_and_size(&luma, &median);
    assert_gpu_packed_luma_order_statistic_rejects_rank_and_chain(&luma, &median);
    assert_gpu_packed_luma_order_statistic_rejects_other_inputs(&median);
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_order_statistic_accepts(
    luma: &DynamicImage,
    median: &PipelineOp,
    rank: &PipelineOp,
    rank_one: &PipelineOp,
    max_filter: &PipelineOp,
    min_filter: &PipelineOp,
) {
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(median),
        luma,
        None
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(median),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(rank),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(rank_one),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(max_filter),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(min_filter),
        luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_order_statistic_rejects_mode_and_size(
    luma: &DynamicImage,
    median: &PipelineOp,
) {
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(median),
        luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MedianFilter { size: 5 }],
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::RankFilter { size: 7, rank: 20 }],
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MaxFilter { size: 5 }],
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[PipelineOp::MinFilter { size: 5 }],
        luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_order_statistic_rejects_rank_and_chain(
    luma: &DynamicImage,
    median: &PipelineOp,
) {
    for rank in [0, 2, 8] {
        assert!(!super::gpu_packed_luma_order_statistic_input(
            &[PipelineOp::RankFilter { size: 3, rank }],
            luma,
            Some("L")
        ));
    }
    assert!(!super::gpu_packed_luma_order_statistic_input(
        &[(*median).clone(), PipelineOp::Duplicate],
        luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_order_statistic_rejects_other_inputs(median: &PipelineOp) {
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(median),
        &DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6]).unwrap()),
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_order_statistic_input(
        std::slice::from_ref(median),
        &DynamicImage::ImageLuma8(GrayImage::new(0, 1)),
        Some("L")
    ));
}
