use super::*;

#[cfg(target_endian = "little")]
#[test]
fn native_rgb_putalpha_planner_checks_pixel_coverage_and_device_limits() {
    let plan = plan_native_rgb_put_alpha_dispatch(19, 17, 2, u32::MAX, u64::MAX)
        .expect("small RGB image should fit a tiled dispatch");
    assert_eq!((plan.groups_x, plan.groups_y), (2, 2));
    assert_eq!(plan.input_bytes, 19 * 17 * 3);
    assert_eq!(plan.input_transfer_bytes, (19 * 17 * 3 + 3) as u64 & !3);
    assert_eq!(plan.output_bytes, 19 * 17 * 4);
    assert_eq!(plan.output_transfer_bytes, (19 * 17 * 4) as u64);

    let mut writes = vec![0u8; 19 * 17];
    for invocation_y in 0..plan.groups_y * 16 {
        for invocation_x in 0..plan.groups_x * 16 {
            if invocation_x < 19 && invocation_y < 17 {
                writes[(invocation_y * 19 + invocation_x) as usize] += 1;
            }
        }
    }
    assert!(writes.iter().all(|count| *count == 1));

    let image_4k = plan_native_rgb_put_alpha_dispatch(4096, 4096, 256, u32::MAX, u64::MAX)
        .expect("4096x4096 fits the default 2D compute grid");
    assert_eq!((image_4k.groups_x, image_4k.groups_y), (256, 256));
    assert_eq!(image_4k.input_bytes, 4096 * 4096 * 3);
    assert_eq!(image_4k.output_bytes, 4096 * 4096 * 4);

    let max_groups = 65_535;
    let width_at_limit = 16 * max_groups;
    assert!(
        plan_native_rgb_put_alpha_dispatch(width_at_limit, 16, max_groups, u32::MAX, u64::MAX,)
            .is_some()
    );
    assert!(
        plan_native_rgb_put_alpha_dispatch(width_at_limit + 1, 16, max_groups, u32::MAX, u64::MAX,)
            .is_none()
    );
    assert!(plan_native_rgb_put_alpha_dispatch(16, 16, 1, 1_000, u64::MAX).is_none());
    assert!(plan_native_rgb_put_alpha_dispatch(16, 16, 1, u32::MAX, 1_000).is_none());
    assert!(
        plan_native_rgb_put_alpha_dispatch(u32::MAX, 2, max_groups, u32::MAX, u64::MAX).is_none()
    );
    assert!(plan_native_rgb_put_alpha_dispatch(0, 16, max_groups, u32::MAX, u64::MAX).is_none());
    assert!(plan_native_rgb_put_alpha_dispatch(16, 16, 0, u32::MAX, u64::MAX).is_none());
}

#[cfg(target_endian = "little")]
#[test]
fn native_rgb_putalpha_data_planner_checks_both_inputs_and_device_limits() {
    let plan = plan_native_rgb_put_alpha_data_dispatch(19, 17, 2, u32::MAX, u64::MAX)
        .expect("small RGB+L image should fit a tiled dispatch");
    assert_eq!((plan.groups_x, plan.groups_y), (2, 2));
    assert_eq!(plan.rgb_bytes, 19 * 17 * 3);
    assert_eq!(plan.rgb_transfer_bytes, (19 * 17 * 3 + 3) as u64 & !3);
    assert_eq!(plan.mask_bytes, 19 * 17);
    assert_eq!(plan.mask_transfer_bytes, (19 * 17 + 3) as u64 & !3);
    assert_eq!(plan.output_bytes, 19 * 17 * 4);
    assert_eq!(plan.output_transfer_bytes, (19 * 17 * 4) as u64);

    let mut writes = vec![0u8; 19 * 17];
    for invocation_y in 0..plan.groups_y * 16 {
        for invocation_x in 0..plan.groups_x * 16 {
            if invocation_x < 19 && invocation_y < 17 {
                writes[(invocation_y * 19 + invocation_x) as usize] += 1;
            }
        }
    }
    assert!(writes.iter().all(|count| *count == 1));

    let image_4k = plan_native_rgb_put_alpha_data_dispatch(4096, 4096, 256, u32::MAX, u64::MAX)
        .expect("4096x4096 fits a 2D grid when buffer limits permit it");
    assert_eq!((image_4k.groups_x, image_4k.groups_y), (256, 256));
    assert_eq!(image_4k.rgb_bytes, 4096 * 4096 * 3);
    assert_eq!(image_4k.mask_bytes, 4096 * 4096);
    assert_eq!(image_4k.output_bytes, 4096 * 4096 * 4);

    let max_groups = 65_535;
    let width_at_limit = 16 * max_groups;
    assert!(
            plan_native_rgb_put_alpha_data_dispatch(
                width_at_limit,
                16,
                max_groups,
                u32::MAX,
                u64::MAX,
            )
            .is_some()
        );
    assert!(
        plan_native_rgb_put_alpha_data_dispatch(
            width_at_limit + 1,
            16,
            max_groups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(plan_native_rgb_put_alpha_data_dispatch(16, 16, 1, 100, u64::MAX).is_none());
    assert!(plan_native_rgb_put_alpha_data_dispatch(16, 16, 1, u32::MAX, 100).is_none());
    assert!(
        plan_native_rgb_put_alpha_data_dispatch(u32::MAX, 2, max_groups, u32::MAX, u64::MAX)
            .is_none()
    );
    assert!(
        plan_native_rgb_put_alpha_data_dispatch(0, 16, max_groups, u32::MAX, u64::MAX).is_none()
    );
    assert!(plan_native_rgb_put_alpha_data_dispatch(16, 16, 0, u32::MAX, u64::MAX).is_none());
}

#[cfg(target_endian = "little")]
#[test]
fn native_rgba_putalpha_data_planner_checks_coverage_and_device_limits() {
    let plan = plan_native_rgba_put_alpha_data_dispatch(19, 17, 2, u32::MAX, u64::MAX)
        .expect("small RGBA+L image should fit a tiled dispatch");
    assert_eq!((plan.groups_x, plan.groups_y), (2, 2));
    assert_eq!(plan.rgba_bytes, 19 * 17 * 4);
    assert_eq!(plan.rgba_transfer_bytes, (19 * 17 * 4) as u64);
    assert_eq!(plan.mask_bytes, 19 * 17);
    assert_eq!(plan.mask_transfer_bytes, (19 * 17 + 3) as u64 & !3);
    assert_eq!(plan.output_transfer_bytes, plan.rgba_transfer_bytes);

    let mut writes = vec![0u8; 19 * 17];
    for invocation_y in 0..plan.groups_y * 16 {
        for invocation_x in 0..plan.groups_x * 16 {
            if invocation_x < 19 && invocation_y < 17 {
                writes[(invocation_y * 19 + invocation_x) as usize] += 1;
            }
        }
    }
    assert!(writes.iter().all(|count| *count == 1));

    let image_4k = plan_native_rgba_put_alpha_data_dispatch(4096, 4096, 256, u32::MAX, u64::MAX)
        .expect("4096x4096 fits a 2D grid when buffer limits permit it");
    assert_eq!((image_4k.groups_x, image_4k.groups_y), (256, 256));
    assert_eq!(image_4k.rgba_bytes, 4096 * 4096 * 4);
    assert_eq!(image_4k.mask_bytes, 4096 * 4096);

    let max_groups = 65_535;
    let width_at_limit = 16 * max_groups;
    assert!(
        plan_native_rgba_put_alpha_data_dispatch(
            width_at_limit,
            16,
            max_groups,
            u32::MAX,
            u64::MAX,
        )
        .is_some()
    );
    assert!(
        plan_native_rgba_put_alpha_data_dispatch(
            width_at_limit + 1,
            16,
            max_groups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(plan_native_rgba_put_alpha_data_dispatch(16, 16, 1, 100, u64::MAX).is_none());
    assert!(plan_native_rgba_put_alpha_data_dispatch(16, 16, 1, u32::MAX, 100).is_none());
    assert!(
        plan_native_rgba_put_alpha_data_dispatch(u32::MAX, 2, max_groups, u32::MAX, u64::MAX)
            .is_none()
    );
    assert!(
        plan_native_rgba_put_alpha_data_dispatch(0, 16, max_groups, u32::MAX, u64::MAX).is_none()
    );
    assert!(plan_native_rgba_put_alpha_data_dispatch(16, 16, 0, u32::MAX, u64::MAX).is_none());
}

#[test]
fn packed_point_luma_dispatch_respects_device_limit() {
    assert_eq!(
        plan_packed_point_luma_dispatch(4096, 4096, 65_535)
            .expect("4K luma image fits the packed Point dispatch"),
        (16_384, 1)
    );
    let max_pixels = 4 * super::PACKED_LUMA_WORKGROUP_SIZE * 65_535;
    assert_eq!(
        plan_packed_point_luma_dispatch(max_pixels as u32, 1, 65_535)
            .expect("the adapter boundary should be admitted"),
        (65_535, 1)
    );
    assert!(plan_packed_point_luma_dispatch(max_pixels as u32 + 1, 1, 65_535).is_err());
    assert!(plan_packed_point_luma_dispatch(1, 1, 0).is_err());
    assert!(plan_packed_point_luma_dispatch(0, 1, 65_535).is_err());
}

#[test]
fn putdata_resource_estimate_matches_packed_luma_auxiliary_words() {
    assert_eq!(putdata_auxiliary_words(1_273, 1, false), 1_273);
    assert_eq!(putdata_auxiliary_words(1_273, 1, true), 319);
    assert_eq!(putdata_auxiliary_words(1_257, 1, true), 315);
    assert_eq!(putdata_auxiliary_words(0, 1, true), 0);
    assert_eq!(putdata_auxiliary_words(3_819, 3, false), 1_273);
}

#[test]
fn blur_dispatch_planner_bounds_grid_and_covers_each_pixel_once() {
    let max_groups = 65_535;
    let tile = BLUR_WORKGROUP_SIZE;
    assert_eq!(
        plan_blur_dispatch(tile * max_groups, 1, max_groups).expect("nonzero grid limit"),
        (max_groups, 1)
    );
    assert_eq!(
        plan_blur_dispatch(tile * max_groups + 1, tile * max_groups + 1, max_groups)
            .expect("nonzero grid limit"),
        (max_groups, max_groups)
    );
    assert!(plan_blur_dispatch(1, 1, 0).is_err());

    for (width, height) in [(1, 1), (16, 17), (17, 16), (33, 35), (65, 47)] {
        let (groups_x, groups_y) =
            plan_blur_dispatch(width, height, 2).expect("nonzero grid limit");
        assert!(groups_x <= 2 && groups_y <= 2);
        let stride_x = groups_x * tile;
        let stride_y = groups_y * tile;
        let mut writes = vec![0u8; (width * height) as usize];
        for invocation_y in 0..groups_y * tile {
            let mut y = invocation_y;
            while y < height {
                for invocation_x in 0..groups_x * tile {
                    let mut x = invocation_x;
                    while x < width {
                        writes[(y * width + x) as usize] += 1;
                        x += stride_x;
                    }
                }
                y += stride_y;
            }
        }
        assert!(
            writes.iter().all(|count| *count == 1),
            "{width}x{height} dispatch must cover every pixel exactly once"
        );
    }
}

#[test]
fn blur_dispatch_dimensions_allow_grid_stride_past_device_grid() {
    let op = PipelineOp::BoxBlur { radius: 1 };
    assert!(!gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&op),
        (65, 33),
        4,
        Some("RGB"),
        false,
    ));
}

#[test]
fn packed_luma_pad_dispatch_uses_its_compact_grid_limit() {
    let op = PipelineOp::Pad {
        w: 1_048_576,
        h: 3,
        filter: ResampleFilter::Bicubic,
        color: None,
        centering: (0.5, 0.5),
    };
    let dimensions = (1_048_576, 2);

    // The 16x16 generic Pad grid needs 65,536 groups across, but the
    // admitted packed-L grid needs only 3,072 groups for these 3,145,728
    // output pixels and therefore fits a 65,535-group adapter limit.
    assert!(gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&op),
        dimensions,
        65_535,
        Some("L"),
        false,
    ));
    assert!(!gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&op),
        dimensions,
        65_535,
        Some("L"),
        true,
    ));
    assert!(gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&op),
        dimensions,
        3_071,
        Some("L"),
        true,
    ));
}

#[test]
fn blur_shader_work_estimate_scales_with_radius() {
    let dimensions = (4096, 4096);
    let small = PipelineOp::GaussianBlur { sigma: 2.0 };
    let larger = PipelineOp::GaussianBlur { sigma: 3.0 };
    let small_work = gpu_shader_work_items(&small, dimensions, dimensions, Some("RGB"))
        .expect("bounded small Gaussian blur");
    let larger_work = gpu_shader_work_items(&larger, dimensions, dimensions, Some("RGB"))
        .expect("bounded larger Gaussian blur");
    assert!(larger_work > small_work);
    assert!(!gpu_shader_work_requires_cpu(
        &small,
        dimensions,
        dimensions,
        Some("RGB")
    ));
    assert!(gpu_shader_work_requires_cpu(
        &larger,
        dimensions,
        dimensions,
        Some("RGB")
    ));
}
