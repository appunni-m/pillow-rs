use super::{
    GPU_BUFFER_CAPACITY, GPU_MASKED_RGBA_WORKGROUP_SIZE, plan_gpu_native_masked_byte_paste,
    plan_gpu_native_masked_l_paste,
};

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
    assert_native_masked_la_paste_workgroup_limit(max_workgroups);
    assert_native_masked_la_paste_global_limit();
    assert_native_masked_la_paste_odd_pixel_transfers(max_workgroups);
    assert_native_masked_la_paste_rejects_invalid_cases(max_workgroups);
}

fn assert_native_masked_la_paste_workgroup_limit(max_workgroups: u32) {
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
}

fn assert_native_masked_la_paste_global_limit() {
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
}

fn assert_native_masked_la_paste_odd_pixel_transfers(max_workgroups: u32) {
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
}

fn assert_native_masked_la_paste_rejects_invalid_cases(max_workgroups: u32) {
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
