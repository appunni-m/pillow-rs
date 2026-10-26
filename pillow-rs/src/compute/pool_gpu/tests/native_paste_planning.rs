use super::*;

#[test]
fn pack_dynamic_rgba_uses_existing_rgba_storage_and_preserves_other_modes() {
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![1, 2, 3, 4, 250, 129, 8, 255]).unwrap(),
    );
    assert_eq!(
        super::pack_dynamic_rgba(&rgba, 2).unwrap(),
        [0x0403_0201, 0xff08_81fa]
    );

    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![7, 11, 13]).unwrap());
    assert_eq!(
        super::pack_dynamic_rgba(&rgb, 1).unwrap(),
        super::pack_rgba(&rgb.to_rgba8(), 1).unwrap()
    );

    let mut arena = vec![0xdead_beef];
    let range = super::append_dynamic_rgba(&mut arena, &rgba, 2, 16).unwrap();
    assert_eq!(range.offset, 16);
    assert_eq!(range.size, 16);
    assert_eq!(arena[0], 0xdead_beef);
    assert_eq!(&arena[4..6], &[0x0403_0201, 0xff08_81fa]);
    assert_eq!(&arena[6..], &[0, 0]);
}

#[test]
fn put_alpha_output_keeps_native_results_and_consumes_rgba_fallback() {
    let bytes = vec![17, 201, 83, 149];
    for mode in [PixelMode::LA, PixelMode::PA] {
        let image =
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(2, 1, bytes.clone()).unwrap());
        let result = super::put_alpha_output(image, mode).unwrap();
        assert!(matches!(&result, DynamicImage::ImageLumaA8(_)));
        assert_eq!(result.as_bytes(), bytes);
    }

    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 1, vec![17, 22, 33, 201, 83, 91, 102, 149]).unwrap(),
    );
    let output = super::put_alpha_output(rgba, PixelMode::L).unwrap();
    assert!(matches!(&output, DynamicImage::ImageLumaA8(_)));
    assert_eq!(output.as_bytes(), [17, 201, 83, 149]);
}

#[test]
fn native_byte_paste_planner_checks_l_la_rgb_and_adapter_boundaries() {
    let max_workgroups = 65_535;
    assert_native_l_byte_paste_boundary(max_workgroups);
    assert_native_la_byte_paste_boundary(max_workgroups);
    assert_native_rgb_byte_paste_boundary(max_workgroups);
    assert_native_byte_paste_adapter_rejections(max_workgroups);
}

fn assert_native_l_byte_paste_boundary(max_workgroups: u32) {
    let l_max_pixels = u64::from(max_workgroups) * 64 * 4;
    let l_max_width = u32::try_from(l_max_pixels).expect("L boundary fits image dimensions");
    let l_at_limit =
        plan_gpu_native_byte_paste(l_max_width, 1, 1, 1, 1, max_workgroups, u32::MAX, u64::MAX)
            .expect("last L output word fits the adapter limit");
    assert_eq!(l_at_limit.word_count, max_workgroups * 64);
    assert_eq!(l_at_limit.workgroups, max_workgroups);
    assert_eq!(
        l_at_limit.destination_transfer_bytes, l_max_pixels,
        "native L output remains byte packed",
    );

    assert!(
        plan_gpu_native_byte_paste(
            l_max_width + 1,
            1,
            1,
            1,
            1,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
}

fn assert_native_la_byte_paste_boundary(max_workgroups: u32) {
    let la_max_pixels = u64::from(max_workgroups) * 64 * 2;
    let la_max_width = u32::try_from(la_max_pixels).expect("LA boundary fits image dimensions");
    let la_at_limit =
        plan_gpu_native_byte_paste(la_max_width, 1, 1, 1, 2, max_workgroups, u32::MAX, u64::MAX)
            .expect("last LA output word fits the adapter limit");
    assert_eq!(la_at_limit.word_count, max_workgroups * 64);
    assert_eq!(la_at_limit.workgroups, max_workgroups);
    assert_eq!(
        la_at_limit.destination_transfer_bytes,
        la_max_pixels * 2,
        "native LA keeps both bytes per pixel",
    );
    assert!(
        plan_gpu_native_byte_paste(
            la_max_width + 1,
            1,
            1,
            1,
            2,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
}

fn assert_native_rgb_byte_paste_boundary(max_workgroups: u32) {
    let rgb_max_transfer = u64::from(max_workgroups) * 64 * 4;
    let rgb_max_width =
        u32::try_from(rgb_max_transfer / 3).expect("RGB boundary fits image dimensions");
    assert_eq!(u64::from(rgb_max_width) * 3, rgb_max_transfer);
    let rgb_at_limit = plan_gpu_native_byte_paste(
        rgb_max_width,
        1,
        1,
        1,
        3,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("last RGB output word fits the adapter limit");
    assert_eq!(rgb_at_limit.word_count, max_workgroups * 64);
    assert_eq!(rgb_at_limit.workgroups, max_workgroups);
    assert_eq!(rgb_at_limit.destination_bytes as u64, rgb_max_transfer);
    assert_eq!(rgb_at_limit.destination_transfer_bytes, rgb_max_transfer);
    assert!(
        plan_gpu_native_byte_paste(
            rgb_max_width + 1,
            1,
            1,
            1,
            3,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
}

fn assert_native_byte_paste_adapter_rejections(max_workgroups: u32) {
    assert!(plan_gpu_native_byte_paste(8, 1, 1, 1, 1, max_workgroups, 7, u64::MAX,).is_none());
    assert!(plan_gpu_native_byte_paste(8, 1, 1, 1, 1, max_workgroups, u32::MAX, 7,).is_none());
    assert!(
        plan_gpu_native_byte_paste(8, 1, 1, 1, 4, max_workgroups, u32::MAX, u64::MAX,).is_none()
    );
    assert!(
        plan_gpu_native_byte_paste(0, 1, 1, 1, 1, max_workgroups, u32::MAX, u64::MAX,).is_none()
    );
}

#[test]
fn native_rgb_crop_planner_bounds_packed_tails_and_adapter_limits() {
    let odd_tail = plan_gpu_native_rgb_crop(5, 3, 1, 1, 4, 3, 65_535, u32::MAX, u64::MAX)
        .expect("small RGB crop with a partial output word");
    assert_eq!(odd_tail.source_bytes, 45);
    assert_eq!(odd_tail.source_transfer_bytes, 48);
    assert_eq!(odd_tail.output_width, 3);
    assert_eq!(odd_tail.output_height, 2);
    assert_eq!(odd_tail.output_bytes, 18);
    assert_eq!(odd_tail.output_transfer_bytes, 20);
    assert_eq!(odd_tail.output_word_count, 5);
    assert_eq!(odd_tail.output_pixel_groups, 2);
    assert_eq!(odd_tail.buffer_words, 12);
    assert_eq!(odd_tail.workgroups, 1);

    let at_workgroup_limit = plan_gpu_native_rgb_crop(256, 1, 0, 0, 256, 1, 1, u32::MAX, u64::MAX)
        .expect("64 four-pixel groups fit one workgroup");
    assert_eq!(at_workgroup_limit.output_pixel_groups, 64);
    assert_eq!(at_workgroup_limit.workgroups, 1);
    assert!(plan_gpu_native_rgb_crop(257, 1, 0, 0, 257, 1, 1, u32::MAX, u64::MAX).is_none());

    assert!(plan_gpu_native_rgb_crop(5, 3, 1, 1, 4, 3, 65_535, 47, u64::MAX).is_none());
    assert!(plan_gpu_native_rgb_crop(5, 3, 1, 1, 4, 3, 65_535, u32::MAX, 47).is_none());
    assert!(
        plan_gpu_native_rgb_crop(
            GPU_BUFFER_CAPACITY + 1,
            1,
            0,
            0,
            1,
            1,
            65_535,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(plan_gpu_native_rgb_crop(0, 1, 0, 0, 0, 1, 65_535, u32::MAX, u64::MAX).is_none());
    assert!(plan_gpu_native_rgb_crop(5, 3, 4, 0, 4, 2, 65_535, u32::MAX, u64::MAX).is_none());
}

#[test]
fn native_rgb_to_rgba_paste_planner_bounds_mixed_layout_and_2d_dispatch() {
    let max_workgroups = 65_535;
    let at_boundary_pixels = u64::from(max_workgroups) * 64;
    let boundary_width = u32::try_from(at_boundary_pixels).unwrap();
    let at_boundary = plan_gpu_native_rgb_to_rgba_paste(
        boundary_width,
        1,
        1,
        1,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("the last one-dimensional dispatch group is representable");
    assert_eq!(at_boundary.workgroups_x, max_workgroups);
    assert_eq!(at_boundary.workgroups_y, 1);
    assert_eq!(at_boundary.destination_bytes as u64, at_boundary_pixels * 4);
    assert_eq!(at_boundary.source_bytes, 3);
    assert_eq!(at_boundary.source_transfer_bytes, 4);
    assert_eq!(
        at_boundary.input_transfer_bytes,
        at_boundary.destination_transfer_bytes + at_boundary.source_transfer_bytes
    );

    let just_over = plan_gpu_native_rgb_to_rgba_paste(
        boundary_width + 1,
        1,
        1,
        1,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("one extra output group is represented on a second dispatch row");
    assert_eq!(just_over.workgroups_x, max_workgroups);
    assert_eq!(just_over.workgroups_y, 2);

    let image_4096 = plan_gpu_native_rgb_to_rgba_paste(
        4096,
        4096,
        4096,
        4096,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("a 4096-square output uses a bounded 2D dispatch");
    assert_eq!(image_4096.destination_pixels, 4096 * 4096);
    assert_eq!(image_4096.workgroups_x, max_workgroups);
    assert_eq!(image_4096.workgroups_y, 5);
    assert_eq!(image_4096.destination_transfer_bytes, 4096 * 4096 * 4);
    assert_eq!(image_4096.source_transfer_bytes, 4096 * 4096 * 3);

    assert!(
        plan_gpu_native_rgb_to_rgba_paste(boundary_width, 1, 1, 1, 1, u32::MAX, u64::MAX,)
            .is_none()
    );
    assert!(plan_gpu_native_rgb_to_rgba_paste(8, 1, 1, 1, max_workgroups, 31, u64::MAX,).is_none());
    assert!(plan_gpu_native_rgb_to_rgba_paste(8, 1, 1, 1, max_workgroups, u32::MAX, 31,).is_none());
    assert!(
        plan_gpu_native_rgb_to_rgba_paste(0, 1, 1, 1, max_workgroups, u32::MAX, u64::MAX,)
            .is_none()
    );
}

#[test]
fn native_masked_l_paste_planner_checks_packed_workgroup_and_buffer_limits() {
    let max_workgroups = 65_535;
    let max_pixels = max_workgroups * 64 * 4;
    let at_limit = plan_gpu_native_masked_l_paste(
        max_pixels,
        1,
        4,
        1,
        4,
        1,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("last packed L workgroup fits the adapter limit");
    assert_eq!(at_limit.destination_pixels, max_pixels);
    assert_eq!(at_limit.source_pixels, 4);
    assert_eq!(at_limit.word_count, max_workgroups * 64);
    assert_eq!(at_limit.workgroups, max_workgroups);
    assert_eq!(at_limit.destination_transfer_bytes, u64::from(max_pixels));
    assert_eq!(at_limit.input_transfer_bytes, 8);

    assert!(
        plan_gpu_native_masked_l_paste(
            max_pixels + 1,
            1,
            4,
            1,
            4,
            1,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        plan_gpu_native_masked_l_paste(5, 1, 4, 1, 3, 1, max_workgroups, u32::MAX, u64::MAX,)
            .is_none()
    );
    assert!(
        plan_gpu_native_masked_l_paste(5, 1, 4, 1, 4, 1, max_workgroups, 7, u64::MAX,).is_none()
    );
    assert!(
        plan_gpu_native_masked_l_paste(5, 1, 4, 1, 4, 1, max_workgroups, u32::MAX, 7,).is_none()
    );
    // The coalesced source/mask binding is larger than either input alone.
    assert!(
        plan_gpu_native_masked_l_paste(1, 1, 2, 1, 2, 1, max_workgroups, 4, u64::MAX,).is_none()
    );
    assert!(
        plan_gpu_native_masked_l_paste(1, 1, 2, 1, 2, 1, max_workgroups, u32::MAX, 7,).is_none()
    );
    assert!(
        plan_gpu_native_masked_l_paste(0, 1, 4, 1, 4, 1, max_workgroups, u32::MAX, u64::MAX,)
            .is_none()
    );
}

#[test]
fn native_masked_la_paste_planner_checks_two_pixel_packing_and_limits() {
    let max_workgroups = 65_535;
    let max_pixels = max_workgroups * 64 * 2;
    let at_limit = plan_gpu_native_masked_byte_paste(
        max_pixels,
        1,
        2,
        1,
        2,
        1,
        2,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("last packed LA workgroup fits the adapter limit");
    assert_eq!(at_limit.destination_pixels, max_pixels);
    assert_eq!(at_limit.source_pixels, 2);
    assert_eq!(at_limit.bytes_per_pixel, 2);
    assert_eq!(at_limit.word_count, max_workgroups * 64);
    assert_eq!(at_limit.workgroups, max_workgroups);
    assert_eq!(at_limit.destination_bytes, max_pixels as usize * 2);
    assert_eq!(
        at_limit.destination_transfer_bytes,
        u64::from(max_pixels) * 2
    );

    assert!(
        plan_gpu_native_masked_byte_paste(
            max_pixels + 1,
            1,
            2,
            1,
            2,
            1,
            2,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        plan_gpu_native_masked_byte_paste(
            GPU_BUFFER_CAPACITY + 1,
            1,
            1,
            1,
            1,
            1,
            2,
            u32::MAX,
            u32::MAX,
            u64::MAX,
        )
        .is_none(),
        "the compact route must retain the shared static image pixel cap"
    );

    let odd_pixels =
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 5, 3, 2, max_workgroups, u32::MAX, u64::MAX)
            .expect("odd pixel counts have one padded output word");
    assert_eq!(odd_pixels.destination_bytes, 30);
    assert_eq!(odd_pixels.destination_transfer_bytes, 32);
    assert_eq!(odd_pixels.source_bytes, 30);
    assert_eq!(odd_pixels.source_transfer_bytes, 32);
    assert_eq!(odd_pixels.mask_bytes, 15);
    assert_eq!(odd_pixels.mask_transfer_bytes, 16);
    assert_eq!(odd_pixels.input_transfer_bytes, 48);
    assert_eq!(odd_pixels.word_count, 8);
    assert_eq!(odd_pixels.workgroups, 1);

    assert!(
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 5, 3, 2, max_workgroups, 47, u64::MAX,)
            .is_none()
    );
    assert!(
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 4, 3, 2, max_workgroups, u32::MAX, u64::MAX,)
            .is_none()
    );
    assert!(
        plan_gpu_native_masked_byte_paste(1, 1, 1, 1, 1, 1, 5, max_workgroups, u32::MAX, u64::MAX,)
            .is_none(),
        "unsupported five-byte pixels must remain rejected"
    );
}

#[test]
fn native_masked_rgb_paste_planner_checks_three_byte_words_and_limits() {
    let max_workgroups = 65_535;
    assert_masked_rgb_paste_workgroup_boundary(max_workgroups);
    assert_masked_rgb_paste_global_limit();
    assert_masked_rgb_paste_odd_transfer_padding(max_workgroups);
}

fn assert_masked_rgb_paste_workgroup_boundary(max_workgroups: u32) {
    let max_pixels = max_workgroups * 64 * 4;
    let at_limit = plan_gpu_native_masked_byte_paste(
        max_pixels,
        1,
        4,
        1,
        4,
        1,
        3,
        max_workgroups,
        u32::MAX,
        u64::MAX,
    )
    .expect("last four-pixel RGB workgroup fits the adapter limit");
    assert_eq!(at_limit.destination_pixels, max_pixels);
    assert_eq!(at_limit.source_pixels, 4);
    assert_eq!(at_limit.bytes_per_pixel, 3);
    assert_eq!(at_limit.destination_bytes, max_pixels as usize * 3);
    assert_eq!(
        at_limit.destination_transfer_bytes,
        u64::from(max_pixels) * 3
    );
    assert_eq!(at_limit.word_count, max_workgroups * 64 * 3);
    assert_eq!(at_limit.workgroups, max_workgroups);

    assert!(
        plan_gpu_native_masked_byte_paste(
            max_pixels + 1,
            1,
            4,
            1,
            4,
            1,
            3,
            max_workgroups,
            u32::MAX,
            u64::MAX,
        )
        .is_none(),
        "an extra pixel would exceed the one-dimensional workgroup limit"
    );
}

fn assert_masked_rgb_paste_global_limit() {
    assert!(
        plan_gpu_native_masked_byte_paste(
            GPU_BUFFER_CAPACITY + 1,
            1,
            1,
            1,
            1,
            1,
            3,
            u32::MAX,
            u32::MAX,
            u64::MAX,
        )
        .is_none(),
        "the native route retains the shared static image pixel cap"
    );

    let global_limit = plan_gpu_native_masked_byte_paste(
        GPU_BUFFER_CAPACITY,
        1,
        4,
        1,
        4,
        1,
        3,
        65_536,
        u32::MAX,
        u64::MAX,
    )
    .expect("the global pixel cap remains independently enforced");
    assert_eq!(global_limit.destination_pixels, GPU_BUFFER_CAPACITY);
    assert_eq!(global_limit.workgroups, 65_536);
}

fn assert_masked_rgb_paste_odd_transfer_padding(max_workgroups: u32) {
    let odd_pixels =
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 5, 3, 3, max_workgroups, u32::MAX, u64::MAX)
            .expect("odd RGB byte counts use padded native transfers");
    assert_eq!(odd_pixels.destination_bytes, 45);
    assert_eq!(odd_pixels.destination_transfer_bytes, 48);
    assert_eq!(odd_pixels.source_bytes, 45);
    assert_eq!(odd_pixels.source_transfer_bytes, 48);
    assert_eq!(odd_pixels.mask_bytes, 15);
    assert_eq!(odd_pixels.mask_transfer_bytes, 16);
    assert_eq!(odd_pixels.input_transfer_bytes, 64);
    assert_eq!(odd_pixels.word_count, 12);
    assert_eq!(odd_pixels.workgroups, 1);

    assert!(
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 5, 3, 3, max_workgroups, 63, u64::MAX,)
            .is_none(),
        "the coalesced RGB source/mask binding needs 64 padded bytes"
    );
    assert!(
        plan_gpu_native_masked_byte_paste(5, 3, 5, 3, 4, 3, 3, max_workgroups, u32::MAX, u64::MAX,)
            .is_none(),
        "mask and source geometry must agree"
    );
    assert!(
        plan_gpu_native_masked_byte_paste(0, 1, 1, 1, 1, 1, 3, max_workgroups, u32::MAX, u64::MAX,)
            .is_none()
    );
}

#[test]
fn native_masked_rgba_paste_planner_bounds_two_dimensional_dispatch() {
    let x_limit = 65_535u32;
    let x_boundary_pixels = x_limit * GPU_MASKED_RGBA_WORKGROUP_SIZE;
    assert_native_masked_rgba_adapter_axis_limits(x_limit, x_boundary_pixels);
    assert_native_masked_rgba_four_k_dispatch(x_limit);
    assert_native_masked_rgba_small_grid_limit();
}

fn assert_native_masked_rgba_adapter_axis_limits(x_limit: u32, x_boundary_pixels: u32) {
    let one_axis = plan_gpu_native_masked_byte_paste(
        x_boundary_pixels,
        1,
        1,
        1,
        1,
        1,
        4,
        x_limit,
        u32::MAX,
        u64::MAX,
    )
    .expect("RGBA groups fit exactly at the adapter's x-axis limit");
    assert_eq!(one_axis.workgroups, x_limit);
    assert_eq!(one_axis.workgroups_x, x_limit);
    assert_eq!(one_axis.workgroups_y, 1);

    let two_axes = plan_gpu_native_masked_byte_paste(
        x_boundary_pixels + 1,
        1,
        1,
        1,
        1,
        1,
        4,
        x_limit,
        u32::MAX,
        u64::MAX,
    )
    .expect("one more RGBA pixel is safely represented on the second axis");
    assert_eq!(two_axes.workgroups, 65_536);
    assert_eq!(two_axes.workgroups_x, 256);
    assert_eq!(two_axes.workgroups_y, 256);
}

fn assert_native_masked_rgba_four_k_dispatch(x_limit: u32) {
    let four_k = plan_gpu_native_masked_byte_paste(
        4096,
        4096,
        4096,
        4096,
        4096,
        4096,
        4,
        x_limit,
        u32::MAX,
        u64::MAX,
    )
    .expect("4096×4096 native RGBA buffers and dispatch are representable");
    assert_eq!(four_k.destination_pixels, GPU_BUFFER_CAPACITY);
    assert_eq!(four_k.destination_bytes, 4096usize * 4096 * 4);
    assert_eq!(four_k.destination_transfer_bytes, 67_108_864);
    assert_eq!(four_k.input_transfer_bytes, 83_886_080);
    assert_eq!(four_k.workgroups, 262_144);
    assert_eq!(four_k.workgroups_x, 512);
    assert_eq!(four_k.workgroups_y, 512);
}

fn assert_native_masked_rgba_small_grid_limit() {
    let four_groups =
        plan_gpu_native_masked_byte_paste(256, 1, 1, 1, 1, 1, 4, 2, u32::MAX, u64::MAX)
            .expect("the last group at a two-dimensional boundary fits");
    assert_eq!((four_groups.workgroups_x, four_groups.workgroups_y), (2, 2));
    assert!(
        plan_gpu_native_masked_byte_paste(257, 1, 1, 1, 1, 1, 4, 2, u32::MAX, u64::MAX,).is_none(),
        "planner rejects a third y group when the adapter allows only two"
    );
    assert!(
        plan_gpu_native_masked_byte_paste(65, 1, 1, 1, 1, 1, 4, 0, u32::MAX, u64::MAX,).is_none(),
        "zero adapter workgroup limit cannot divide or dispatch"
    );
}

#[test]
fn compact_masked_la_paste_can_fit_when_generic_grids_and_buffers_do_not() {
    let source = Arc::new(crate::image::Image::new(17, 1, "LA", (0, 0, 0, 255)).unwrap());
    let mask = Arc::new(crate::image::Image::new(17, 1, "L", (127, 0, 0, 0)).unwrap());
    let operation = PipelineOp::Paste {
        source,
        x: 0,
        y: 0,
        w: 17,
        h: 1,
        mask: Some(mask),
        mask_alpha: false,
    };

    assert!(gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&operation),
        (17, 1),
        1,
        Some("LA"),
        false,
    ));
    assert!(gpu_buffer_capacity_exceeds_limits(17, 56, 56));

    let compact = plan_gpu_native_masked_byte_paste(17, 1, 17, 1, 17, 1, 2, 1, 56, 56)
        .expect("compact LA transport fits both adapter bindings and its grid");
    assert_eq!(compact.workgroups, 1);
    assert_eq!(compact.destination_transfer_bytes, 36);
    assert_eq!(compact.input_transfer_bytes, 56);
    assert!(!gpu_buffer_capacity_exceeds_limits(
        compact.word_count,
        56,
        56
    ));
}

#[test]
fn native_masked_pa_paste_uses_a_distinct_logical_mode_shader() {
    let pa = gpu_native_masked_byte_paste_shader("PA", 2)
        .expect("PA selects its native two-byte pipeline");
    let la = gpu_native_masked_byte_paste_shader("LA", 2)
        .expect("LA selects its own native two-byte pipeline");
    assert_eq!(pa.0, "__internal_paste_native_masked_pa");
    assert_eq!(pa.1, "paste_native_masked_pa.wgsl");
    assert!(pa.2.contains("PA index and alpha are blended independently"));
    assert_eq!(la.0, "__internal_paste_native_masked_la");
    assert_ne!(pa.0, la.0);
    assert!(gpu_native_masked_byte_paste_shader("PA", 3).is_none());
}

#[test]
fn native_masked_rgba_paste_selects_its_native_four_byte_shader() {
    let rgba =
        gpu_native_masked_byte_paste_shader("RGBA", 4).expect("RGBA selects its four-byte shader");
    assert_eq!(rgba.0, "__internal_paste_native_masked_rgba");
    assert_eq!(rgba.1, "paste_native_masked_rgba.wgsl");
    assert!(
        rgba.2
            .contains("word_index = gid.x + gid.y * params.workgroups_x")
    );
    assert!(gpu_native_masked_byte_paste_shader("RGBA", 3).is_none());
    assert!(gpu_native_masked_byte_paste_shader("RGBa", 4).is_none());
}

#[test]
fn native_masked_pa_layout_keeps_the_palette_alpha_mode_tag() {
    let bytes = [1, 2, 3, 4, 5, 6];
    let source_image = Arc::new(Image::frombytes("PA", (3, 1), &bytes).unwrap());
    let mask_image = Arc::new(Image::frombytes("L", (3, 1), &[0, 127, 255]).unwrap());
    let destination =
        DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(3, 1, bytes.to_vec()).unwrap());
    let source = DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(3, 1, bytes.to_vec()).unwrap());
    let mask = DynamicImage::ImageLuma8(GrayImage::from_raw(3, 1, vec![0, 127, 255]).unwrap());
    let op = PipelineOp::Paste {
        source: source_image,
        x: 0,
        y: 0,
        w: 3,
        h: 1,
        mask: Some(mask_image),
        mask_alpha: false,
    };

    assert_eq!(
        gpu_native_masked_byte_paste_layout(&op, &destination, &source, &mask, Some("PA")),
        Some(((0, 0), 2, "PA"))
    );
}

#[test]
fn native_masked_rgba_layout_keeps_four_stored_channels() {
    let source_bytes = [1u8; 5 * 3 * 4];
    let destination_bytes = [2u8; 7 * 4 * 4];
    let mask_bytes = [127u8; 5 * 3];
    let source_image = Arc::new(Image::frombytes("RGBA", (5, 3), &source_bytes).unwrap());
    let mask_image = Arc::new(Image::frombytes("L", (5, 3), &mask_bytes).unwrap());
    let destination =
        DynamicImage::ImageRgba8(RgbaImage::from_raw(7, 4, destination_bytes.to_vec()).unwrap());
    let source =
        DynamicImage::ImageRgba8(RgbaImage::from_raw(5, 3, source_bytes.to_vec()).unwrap());
    let mask = DynamicImage::ImageLuma8(GrayImage::from_raw(5, 3, mask_bytes.to_vec()).unwrap());
    let op = PipelineOp::Paste {
        source: source_image,
        x: -1,
        y: 1,
        w: 5,
        h: 3,
        mask: Some(mask_image),
        mask_alpha: false,
    };

    assert_eq!(
        gpu_native_masked_byte_paste_layout(&op, &destination, &source, &mask, Some("RGBA")),
        Some(((-1, 1), 4, "RGBA"))
    );
}

#[test]
#[cfg(target_endian = "little")]
fn native_masked_rgb_and_hsv_paste_use_the_three_byte_shader() {
    if !native_masked_rgb_gpu_available_or_skip() {
        return;
    }
    let _restore = RestoreGpuDiagnostics::enable();
    let _ = Backend::take_gpu_shader_coverage();
    let (destination_bytes, source_bytes, mask_bytes) = native_masked_rgb_hsv_fixture_bytes();
    for mode in ["RGB", "HSV"] {
        for position in [(-1, -1), (2, 1)] {
            assert_native_masked_rgb_hsv_case(
                mode,
                position,
                &destination_bytes,
                &source_bytes,
                &mask_bytes,
            );
        }
    }
}

#[cfg(target_endian = "little")]
struct RestoreGpuDiagnostics {
    telemetry: bool,
    shader_coverage: bool,
}

#[cfg(target_endian = "little")]
impl RestoreGpuDiagnostics {
    fn enable() -> Self {
        Self {
            telemetry: Backend::set_pipeline_telemetry_enabled(true),
            shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
        }
    }
}

#[cfg(target_endian = "little")]
impl Drop for RestoreGpuDiagnostics {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.telemetry);
        Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
    }
}

#[cfg(target_endian = "little")]
fn native_masked_rgb_gpu_available_or_skip() -> bool {
    match super::GpuPool::ensure_init() {
        Ok(_) => true,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            false
        }
        Err(error) => panic!("native masked RGB GPU initialization failed: {error}"),
    }
}

#[cfg(target_endian = "little")]
fn native_masked_rgb_hsv_fixture_bytes() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut destination_bytes = (0..45)
        .map(|index: i32| (index.wrapping_mul(41).wrapping_add(19)) as u8)
        .collect::<Vec<_>>();
    let mut source_bytes = (0..45)
        .map(|index: i32| 231i32.wrapping_sub(index.wrapping_mul(17)) as u8)
        .collect::<Vec<_>>();
    for pixel in [0usize, 1, 7, 8] {
        let start = pixel.saturating_mul(3);
        let end = start.saturating_add(3);
        destination_bytes[start..end].copy_from_slice(&[100, 100, 100]);
    }
    for pixel in [0usize, 1, 6, 7] {
        let start = pixel.saturating_mul(3);
        let end = start.saturating_add(3);
        source_bytes[start..end].copy_from_slice(&[101, 99, 100]);
    }
    let mut mask_bytes = vec![
        0, 1, 127, 128, 254, 255, 127, 128, 127, 128, 254, 255, 254, 1, 127,
    ];
    mask_bytes[0..2].copy_from_slice(&[127, 128]);
    (destination_bytes, source_bytes, mask_bytes)
}

#[cfg(target_endian = "little")]
fn assert_native_masked_rgb_hsv_case(
    mode: &str,
    position: (i32, i32),
    destination_bytes: &[u8],
    source_bytes: &[u8],
    mask_bytes: &[u8],
) {
    use crate::ops::paste::PasteSource;

    let source = Image::frombytes(mode, (5, 3), source_bytes).unwrap();
    let mask = Image::frombytes("L", (5, 3), mask_bytes).unwrap();
    let mut expected_image = Image::frombytes(mode, (5, 3), destination_bytes).unwrap();
    expected_image
        .paste_at(
            PasteSource::Image(Box::new(source.clone())),
            Some(position),
            Some(&mask),
        )
        .unwrap();
    let expected = expected_image
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU masked Paste reference");

    let mut actual_image = Image::frombytes(mode, (5, 3), destination_bytes).unwrap();
    actual_image
        .paste_at(
            PasteSource::Image(Box::new(source)),
            Some(position),
            Some(&mask),
        )
        .unwrap();
    let actual = actual_image
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU masked Paste");
    assert_eq!(actual, expected, "masked {mode} Paste at {position:?}");
    assert_native_masked_paste_receipt();
}

#[cfg(target_endian = "little")]
fn assert_native_masked_paste_receipt() {
    let receipt = Backend::take_pipeline_telemetry()
        .expect("native masked Paste must publish a telemetry receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native masked GPU resources");
    assert_eq!(resources.mode_conversion_count, 0);
    assert_eq!(resources.upload_bytes, 48 + 48 + 16);
    assert_eq!(resources.auxiliary_bytes, 48 + 16);
    assert_eq!(resources.readback_bytes, 48);

    let shader_dispatches = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_dispatches.len(), 1);
    assert_eq!(
        shader_dispatches[0].variant_name,
        "__internal_paste_native_masked_rgb"
    );
    assert_eq!(
        shader_dispatches[0].shader_file,
        "paste_native_masked_rgb.wgsl"
    );
    assert_eq!(shader_dispatches[0].dispatches, 1);
    assert_eq!(shader_dispatches[0].workgroups, 1);
}
