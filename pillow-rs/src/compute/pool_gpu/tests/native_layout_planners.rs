#[test]
fn packed_luma_resize_dispatch_checks_word_index_and_both_axes() {
    assert_eq!(
        super::plan_packed_luma_resize_dispatch(64, 64, 4).unwrap(),
        (1, 4)
    );
    assert_eq!(
        super::plan_packed_luma_resize_dispatch(1024, 768, 65_535).unwrap(),
        (16, 48)
    );
    assert!(super::plan_packed_luma_resize_dispatch(0, 1, 65_535).is_err());
    assert!(super::plan_packed_luma_resize_dispatch(1, 0, 65_535).is_err());
    assert!(super::plan_packed_luma_resize_dispatch(1, 16, 0).is_err());
    assert!(super::plan_packed_luma_resize_dispatch(64, 80, 4).is_err());
    assert!(super::plan_packed_luma_resize_dispatch(u32::MAX, 2, 65_535).is_err());
}

#[test]
fn gpu_packed_luma_rank_filter_9_work_estimate_tracks_binary_search_passes() {
    let workload = super::gpu_packed_luma_rank_filter_9_work_items((256, 256));
    assert_eq!(workload, 42_467_328);
    assert!(workload < super::MAX_GPU_SHADER_WORK_ITEMS);
    assert_eq!(
        super::gpu_packed_luma_rank_filter_9_work_items((1, 1)),
        2_592
    );

    let maximum_packed_words = super::MAX_GPU_SHADER_WORK_ITEMS / (4 * 8 * 81);
    let maximum_pixels = maximum_packed_words * 4;
    assert!(
        super::gpu_packed_luma_rank_filter_9_work_items((maximum_pixels as u32, 1))
            <= super::MAX_GPU_SHADER_WORK_ITEMS
    );
    assert!(
        super::gpu_packed_luma_rank_filter_9_work_items(((maximum_pixels + 1) as u32, 1))
            > super::MAX_GPU_SHADER_WORK_ITEMS
    );
}
