use super::{
    GPU_BUFFER_CAPACITY, plan_gpu_native_byte_paste, plan_gpu_native_rgb_crop,
    plan_gpu_native_rgb_to_rgba_paste,
};
use crate::pipeline::PixelMode;
use crate::raster::{DynamicImage, GrayAlphaImage, RgbImage, RgbaImage};

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
