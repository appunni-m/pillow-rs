use super::*;

#[test]
fn extract_band_dispatch_planner_tiles_workgroups_without_gaps() {
    for (workgroups, expected) in [
        (65_534u32, (256, 256)),
        (65_535, (256, 256)),
        (65_536, (256, 256)),
    ] {
        let width = workgroups * 256;
        assert_eq!(
            plan_extract_band_dispatch(width, 1, 65_535)
                .expect("boundary image should fit the tiled grid"),
            expected,
            "required groups={workgroups}"
        );
    }

    // A 4096x4096 image requires 65,536 workgroups when flattened, which
    // exceeds the default per-dimension limit by one. The compact 2D grid
    // must remain inside the adapter limit while covering every word.
    assert_eq!(
        plan_extract_band_dispatch(4096, 4096, 65_535)
            .expect("4096x4096 compact output should fit a tiled grid"),
        (256, 256)
    );

    let (groups_x, groups_y) = plan_extract_band_dispatch(5 * 256, 1, 3).expect("small tiled grid");
    assert_eq!((groups_x, groups_y), (3, 2));
    let output_words = 5 * 64;
    let mut covered = vec![false; output_words as usize];
    for group_y in 0..groups_y {
        for group_x in 0..groups_x {
            for lane in 0..64 {
                let word = group_x * 64 + group_y * groups_x * 64 + lane;
                if word < output_words {
                    assert!(
                        !covered[word as usize],
                        "output word {word} was written twice"
                    );
                    covered[word as usize] = true;
                }
            }
        }
    }
    assert!(covered.into_iter().all(|written| written));

    assert!(plan_extract_band_dispatch(5 * 256, 1, 0).is_err());
    assert!(plan_extract_band_dispatch(5 * 256, 1, 1).is_err());
    assert!(plan_extract_band_dispatch(u32::MAX, 2, 65_535).is_err());
}

#[test]
fn native_composite_dispatch_tiles_across_both_workgroup_dimensions() {
    const GROUP_SIZE: u32 = 64;
    const LIMIT: u32 = 65_535;

    assert_eq!(
        super::plan_native_composite_dispatch(LIMIT * GROUP_SIZE, LIMIT),
        Some((LIMIT, 1))
    );
    assert_eq!(
        super::plan_native_composite_dispatch((LIMIT + 1) * GROUP_SIZE, LIMIT),
        Some((LIMIT, 2))
    );
    assert_eq!(
        super::plan_native_composite_dispatch(4 * GROUP_SIZE, 2),
        Some((2, 2))
    );
    assert_eq!(
        super::plan_native_composite_dispatch(5 * GROUP_SIZE, 2),
        None
    );
    assert_eq!(super::plan_native_composite_dispatch(0, LIMIT), None);
    assert_eq!(super::plan_native_composite_dispatch(GROUP_SIZE, 0), None);
}

#[test]
fn sharpness_l_dispatch_selects_safe_row_or_flat_layouts() {
    assert_eq!(
        plan_sharpness_l_dispatch(1024, 768, 65_535).unwrap(),
        super::SharpnessLDispatch {
            groups_x: 16,
            groups_y: 48,
            row_aligned: true,
        }
    );
    assert_eq!(
        plan_sharpness_l_dispatch(4096, 4096, 65_535).unwrap(),
        super::SharpnessLDispatch {
            groups_x: 64,
            groups_y: 256,
            row_aligned: true,
        }
    );
    assert_eq!(
        plan_sharpness_l_dispatch(17, 9, 65_535).unwrap(),
        super::SharpnessLDispatch {
            groups_x: 1,
            groups_y: 1,
            row_aligned: false,
        }
    );

    // If a row grid exceeds an adapter dimension, use the flattened
    // packed-word layout when that bounded grid still fits.
    assert_eq!(
        plan_sharpness_l_dispatch(320, 3, 4).unwrap(),
        super::SharpnessLDispatch {
            groups_x: 1,
            groups_y: 1,
            row_aligned: false,
        }
    );
    assert_eq!(
        plan_sharpness_l_dispatch(4096, 4096, 255).unwrap(),
        super::SharpnessLDispatch {
            groups_x: 128,
            groups_y: 128,
            row_aligned: false,
        }
    );

    let plan = plan_sharpness_l_dispatch(10_239, 1, 4).unwrap();
    assert_eq!((plan.groups_x, plan.groups_y), (4, 3));
    assert!(!plan.row_aligned);
    let output_words = 10_239usize.div_ceil(4);
    let invocation_width = plan.groups_x as usize * 16;
    let mut written = vec![false; output_words];
    for y in 0..plan.groups_y as usize * 16 {
        for x in 0..invocation_width {
            let word = x + y * invocation_width;
            if word < output_words {
                assert!(!written[word], "packed word {word} was written twice");
                written[word] = true;
            }
        }
    }
    assert!(written.into_iter().all(|word| word));

    assert!(plan_sharpness_l_dispatch(1024, 768, 0).is_err());
    assert!(plan_sharpness_l_dispatch(1024, 768, 1).is_err());
    assert!(plan_sharpness_l_dispatch(u32::MAX, 2, 65_535).is_err());
}

#[test]
fn packed_luma_dispatch_respects_workgroup_and_index_boundaries() {
    assert_eq!(
        plan_packed_luma_dispatch(65, 47, 65_535)
            .expect("odd-width GaussianBlur edge case fits the packed grid"),
        (3, 1)
    );
    assert_eq!(
        plan_packed_luma_dispatch(1024, 768, 65_535)
            .expect("material GaussianBlur case fits the packed grid"),
        (768, 1)
    );
    assert_eq!(
        plan_packed_luma_dispatch(4096, 4096, 65_535)
            .expect("4096x4096 L median fits a one-dimensional packed grid"),
        (16_384, 1)
    );

    let max_pixels = 65_535u32 * 256 * 4;
    assert_eq!(
        plan_packed_luma_dispatch(max_pixels, 1, 65_535)
            .expect("the last permitted packed workgroup fits"),
        (65_535, 1)
    );
    assert!(plan_packed_luma_dispatch(max_pixels + 1, 1, 65_535).is_err());
    assert!(plan_packed_luma_dispatch(1, 1, 0).is_err());
    assert!(plan_packed_luma_dispatch(0, 1, 65_535).is_err());
    assert!(plan_packed_luma_dispatch(u32::MAX, 2, 65_535).is_err());
}

#[test]
fn native_expand_output_dispatch_covers_padded_words_within_limits() {
    let plan = plan_native_expand_output_dispatch(600 * 4, 2)
        .expect("small output should fit a tiled dispatch");
    assert_eq!((plan.groups_x, plan.groups_y), (2, 2));
    assert_eq!(plan.words_per_row, 32);
    assert_eq!(plan.word_count, 600);
    assert_eq!(plan.transfer_bytes, 600 * 4);
    let mut writes = vec![0u8; plan.word_count as usize];
    for invocation_y in 0..plan.groups_y * 16 {
        for invocation_x in 0..plan.groups_x * 16 {
            let word = invocation_y * plan.words_per_row + invocation_x;
            if word < plan.word_count {
                writes[word as usize] += 1;
            }
        }
    }
    assert!(writes.iter().all(|count| *count == 1));

    let max_groups = 65_535u64;
    let max_x_words = max_groups * 256;
    let exact_x = plan_native_expand_output_dispatch(max_x_words * 4, 65_535)
        .expect("exact X limit should fit");
    assert_eq!((exact_x.groups_x, exact_x.groups_y), (65_535, 1));
    let next_x = plan_native_expand_output_dispatch((max_x_words + 1) * 4, 65_535)
        .expect("one group beyond X should tile to a second row");
    assert_eq!((next_x.groups_x, next_x.groups_y), (65_535, 2));

    let tiled_limit = 256u64;
    let max_grid_words = tiled_limit * tiled_limit * 256;
    let exact_grid = plan_native_expand_output_dispatch(max_grid_words * 4, 256)
        .expect("exact 2D limit should fit the shader index type");
    assert_eq!((exact_grid.groups_x, exact_grid.groups_y), (256, 256));
    assert!(plan_native_expand_output_dispatch((max_grid_words + 1) * 4, 256).is_none());
    assert!(plan_native_expand_output_dispatch(1, 0).is_none());
    assert!(plan_native_expand_output_dispatch(0, 65_535).is_none());
}

#[test]
fn native_luma_transform_output_plans_packed_words_with_checked_limits() {
    let transform = |w, h, matrix: [f64; 6]| PipelineOp::Transform {
        w,
        h,
        method: TransformMethod::Affine,
        data: Arc::from(matrix.to_vec()),
        filter: ResampleFilter::Nearest,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };

    let odd = [transform(5, 3, [0.0, 1.0, 0.0, 1.0, 0.0, 0.0])];
    let odd_plan = super::plan_native_luma_transform_output(&odd, 1, 16, 16, 4)
        .expect("odd L output fits one packed word grid");
    assert_eq!((odd_plan.groups_x, odd_plan.groups_y), (1, 1));
    assert_eq!(odd_plan.word_count, 4);
    assert_eq!(odd_plan.transfer_bytes, 16);

    let exact_grid = [transform(1024, 4, [0.0, 0.001, 0.0, 0.001, 0.0, 0.0])];
    let exact = super::plan_native_luma_transform_output(&exact_grid, 2, u32::MAX, u64::MAX, 1024)
        .expect("four workgroups fit a 2-by-2 grid");
    assert_eq!((exact.groups_x, exact.groups_y), (2, 2));
    let next_grid = [transform(1025, 4, [0.0, 0.001, 0.0, 0.001, 0.0, 0.0])];
    let next = super::plan_native_luma_transform_output(&next_grid, 3, u32::MAX, u64::MAX, 1025)
        .expect("the fifth workgroup fits on a second grid row");
    assert_eq!((next.groups_x, next.groups_y), (3, 2));

    let image_4096 = [transform(4096, 4096, [0.0, 0.001, 0.0, 0.001, 0.0, 0.0])];
    let large =
        super::plan_native_luma_transform_output(&image_4096, 128, u32::MAX, u64::MAX, 4096 * 4096)
            .expect("4096-square L output has a bounded 2D packed dispatch");
    assert_eq!((large.groups_x, large.groups_y), (128, 128));
    assert_eq!(large.transfer_bytes, 4096 * 4096);

    assert!(super::plan_native_luma_transform_output(&odd, 0, 16, 16, 4).is_none());
    assert!(super::plan_native_luma_transform_output(&odd, 1, 15, 16, 4).is_none());
    assert!(super::plan_native_luma_transform_output(&odd, 1, 16, 15, 4).is_none());
    assert!(super::plan_native_luma_transform_output(&odd, 1, 16, 16, 3).is_none());
    let oversized = [transform(
        65_536,
        65_536,
        [0.0, 0.0001, 0.0, 0.0001, 0.0, 0.0],
    )];
    assert!(
        super::plan_native_luma_transform_output(&oversized, 65_535, u32::MAX, u64::MAX, u32::MAX,)
            .is_none()
    );
}

#[test]
fn native_rgb_packed_output_plans_compact_words_with_checked_limits() {
    let transform = |w, h, matrix: [f64; 6]| PipelineOp::Transform {
        w,
        h,
        method: TransformMethod::Affine,
        data: Arc::from(matrix.to_vec()),
        filter: ResampleFilter::Nearest,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };
    let odd = [transform(5, 3, [0.87, 0.21, -1.25, -0.16, 1.13, 0.5])];
    let odd_plan = super::plan_native_rgb_transform_output(&odd, 1, 48, 48, 15)
        .expect("odd RGB output fits packed byte storage");
    assert_eq!((odd_plan.groups_x, odd_plan.groups_y), (1, 1));
    assert_eq!(odd_plan.output_word_count, 12);
    assert_eq!(odd_plan.transfer_bytes, 48);

    let aligned = [transform(1024, 768, [0.87, 0.21, -1.25, -0.16, 1.13, 0.5])];
    let aligned_plan =
        super::plan_native_rgb_transform_output(&aligned, 128, u32::MAX, u64::MAX, 1024 * 768)
            .expect("aligned RGB output fits the bounded dispatch grid");
    assert_eq!((aligned_plan.groups_x, aligned_plan.groups_y), (16, 48));
    assert!(aligned_plan.row_tiled);
    assert_eq!(aligned_plan.transfer_bytes, 1024 * 768 * 3);

    let image_4k = [transform(4096, 4096, [0.87, 0.21, -1.25, -0.16, 1.13, 0.5])];
    let large =
        super::plan_native_rgb_transform_output(&image_4k, 128, u32::MAX, u64::MAX, 4096 * 4096)
            .expect("4096-square RGB output fits a two-dimensional dispatch");
    assert_eq!((large.groups_x, large.groups_y), (128, 128));
    assert!(!large.row_tiled);
    assert_eq!(large.transfer_bytes, 4096 * 4096 * 3);

    let large_row_tiled =
        super::plan_native_rgb_transform_output(&image_4k, 256, u32::MAX, u64::MAX, 4096 * 4096)
            .expect("4096-square RGB row-tiled output fits when the adapter allows it");
    assert_eq!(
        (large_row_tiled.groups_x, large_row_tiled.groups_y),
        (64, 256)
    );
    assert!(large_row_tiled.row_tiled);

    assert!(super::plan_native_rgb_transform_output(&odd, 0, 48, 48, 15).is_none());
    assert!(super::plan_native_rgb_transform_output(&odd, 1, 47, 48, 15).is_none());
    assert!(super::plan_native_rgb_transform_output(&odd, 1, 48, 47, 15).is_none());
    assert!(super::plan_native_rgb_transform_output(&odd, 1, 48, 48, 11).is_none());
    let oversized = [transform(65_536, 65_536, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0])];
    assert!(
        super::plan_native_rgb_transform_output(&oversized, 65_535, u32::MAX, u64::MAX, u32::MAX,)
            .is_none()
    );
}

#[test]
fn native_rgb_point_output_plans_packed_lut_dispatch_with_checked_limits() {
    let odd = super::plan_native_rgb_point_output(5, 3, 1, 48, 48, 12)
        .expect("odd RGB point output fits one workgroup and padded storage");
    assert_eq!((odd.groups_x, odd.groups_y), (1, 1));
    assert!(!odd.row_tiled);
    assert_eq!(odd.output_word_count, 12);
    assert_eq!(odd.transfer_bytes, 48);

    let aligned =
        super::plan_native_rgb_point_output(1024, 768, 128, u32::MAX, u64::MAX, 1024 * 768)
            .expect("benchmark RGB output fits its bounded dispatch grid");
    assert_eq!((aligned.groups_x, aligned.groups_y), (16, 48));
    assert!(aligned.row_tiled);
    assert_eq!(aligned.transfer_bytes, 1024 * 768 * 3);

    let image_4k =
        super::plan_native_rgb_point_output(4096, 4096, 65_535, u32::MAX, u64::MAX, 4096 * 4096)
            .expect("4K RGB output is below the per-dimension workgroup limit");
    assert_eq!((image_4k.groups_x, image_4k.groups_y), (64, 256));
    assert!(image_4k.row_tiled);
    assert_eq!(image_4k.transfer_bytes, 4096 * 4096 * 3);

    assert!(super::plan_native_rgb_point_output(5, 3, 0, 48, 48, 12).is_none());
    assert!(super::plan_native_rgb_point_output(5, 3, 1, 47, 48, 12).is_none());
    assert!(super::plan_native_rgb_point_output(5, 3, 1, 48, 47, 12).is_none());
    assert!(super::plan_native_rgb_point_output(5, 3, 1, 48, 48, 11).is_none());
    assert!(
        super::plan_native_rgb_point_output(65_536, 65_536, 65_535, u32::MAX, u64::MAX, u32::MAX,)
            .is_none()
    );
}

#[test]
#[cfg(target_endian = "little")]
fn native_la_transform_output_plans_compact_pairs_with_checked_limits() {
    let transform = |w, h, matrix: [f64; 6]| PipelineOp::Transform {
        w,
        h,
        method: TransformMethod::Affine,
        data: Arc::from(matrix.to_vec()),
        filter: ResampleFilter::Nearest,
        fill: Some((173, 127, 0, 0)),
        fill_is_none: false,
        palette_fill: None,
    };
    let affine = [0.87, 0.21, -1.25, -0.16, 1.13, 0.5];
    let odd = [transform(5, 3, affine)];
    let odd_plan = super::plan_native_la_transform_output(&odd, 1, 32, 32, 8)
        .expect("odd LA output fits one compact workgroup");
    assert_eq!((odd_plan.groups_x, odd_plan.groups_y), (1, 1));
    assert!(!odd_plan.row_tiled);
    assert_eq!(odd_plan.output_word_count, 8);
    assert_eq!(odd_plan.transfer_bytes, 32);

    let aligned = [transform(8, 4, affine)];
    let aligned_plan = super::plan_native_la_transform_output(&aligned, 128, 64, 64, 16)
        .expect("aligned LA rows fit compact output");
    assert_eq!((aligned_plan.groups_x, aligned_plan.groups_y), (1, 1));
    assert!(aligned_plan.row_tiled);
    assert_eq!(aligned_plan.output_word_count, 16);
    assert_eq!(aligned_plan.transfer_bytes, 64);

    let image_4k = [transform(4096, 4096, affine)];
    let bounded =
        super::plan_native_la_transform_output(&image_4k, 128, u32::MAX, u64::MAX, u32::MAX)
            .expect("4096-square LA output fits within a 128-by-128 dispatch grid");
    assert_eq!((bounded.groups_x, bounded.groups_y), (128, 128));
    assert!(!bounded.row_tiled);
    assert_eq!(bounded.transfer_bytes, 4096 * 4096 * 2);

    let row_tiled =
        super::plan_native_la_transform_output(&image_4k, 256, u32::MAX, u64::MAX, u32::MAX)
            .expect("4096-square LA rows fit when the adapter allows 256 workgroups per axis");
    assert_eq!((row_tiled.groups_x, row_tiled.groups_y), (64, 256));
    assert!(row_tiled.row_tiled);

    assert!(super::plan_native_la_transform_output(&odd, 0, 32, 32, 8).is_none());
    assert!(super::plan_native_la_transform_output(&odd, 1, 31, 32, 8).is_none());
    assert!(super::plan_native_la_transform_output(&odd, 1, 32, 31, 8).is_none());
    assert!(super::plan_native_la_transform_output(&odd, 1, 32, 32, 7).is_none());
    let wrong_channels = super::plan_native_transform_output(&odd, 4, 1, 32, 32, 8);
    assert!(wrong_channels.is_none());
}
