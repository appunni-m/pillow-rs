use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_transpose_layout_bounds_and_padding() {
    for (pixels, bytes) in [(1, 4), (2, 8), (3, 12), (4, 12), (5, 16), (6, 20)] {
        let layout = super::packed_rgb_transpose_layout(pixels, 1, 1).unwrap();
        assert_eq!(layout.native_bytes, pixels as usize * 3);
        assert_eq!(layout.transfer_bytes, bytes);
        assert_eq!(layout.workgroups, 1);
    }
    assert!(super::packed_rgb_transpose_layout(0, 1, 1).is_none());
    assert!(super::packed_rgb_transpose_layout(1, 0, 1).is_none());
    assert!(super::packed_rgb_transpose_layout(1, 1, 0).is_none());
    assert!(super::packed_rgb_transpose_layout(513, 1, 1).is_none());
    assert_eq!(
        super::packed_rgb_transpose_layout(513, 1, 2)
            .unwrap()
            .workgroups,
        2
    );
    assert!(super::packed_rgb_transpose_layout(u32::MAX, u32::MAX, u32::MAX).is_none());
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_byte_flip_dispatch_bounds_padded_rows_and_grid() {
    assert_native_byte_flip_padded_row_plans();

    let exact_axis = super::plan_native_byte_flip_dispatch(
        256,
        1,
        1,
        1,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!((exact_axis.groups_x, exact_axis.groups_y), (1, 1));
    assert!(
        super::plan_native_byte_flip_dispatch(
            257,
            1,
            1,
            1,
            u32::MAX,
            u64::MAX,
            super::GPU_BUFFER_CAPACITY,
        )
        .is_none()
    );

    let below_default_axis = super::plan_native_byte_flip_dispatch(
        4092,
        4096,
        1,
        65_535,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!(below_default_axis.groups_y, 1);
    let over_flat_axis = super::plan_native_byte_flip_dispatch(
        4096,
        4096,
        1,
        65_535,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!(
        (over_flat_axis.groups_x, over_flat_axis.groups_y),
        (65_535, 2)
    );
    let two_dimensional_grid = super::plan_native_byte_flip_dispatch(
        4097,
        4096,
        1,
        65_535,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!(two_dimensional_grid.groups_y, 2);

    assert_native_byte_flip_rejected_limits();
}

#[cfg(target_endian = "little")]
fn assert_native_byte_flip_padded_row_plans() {
    let luma = super::plan_native_byte_flip_dispatch(
        5,
        3,
        1,
        65_535,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!(luma.row_bytes, 5);
    assert_eq!(luma.words_per_row, 2);
    assert_eq!(luma.word_count, 6);
    assert_eq!(luma.transfer_bytes, 24);
    assert_eq!((luma.groups_x, luma.groups_y), (1, 1));

    let alpha = super::plan_native_byte_flip_dispatch(
        5,
        3,
        2,
        65_535,
        u32::MAX,
        u64::MAX,
        super::GPU_BUFFER_CAPACITY,
    )
    .unwrap();
    assert_eq!(alpha.row_bytes, 10);
    assert_eq!(alpha.words_per_row, 3);
    assert_eq!(alpha.word_count, 9);
    assert_eq!(alpha.transfer_bytes, 36);
}

#[cfg(target_endian = "little")]
fn assert_native_byte_flip_rejected_limits() {
    assert!(
        super::plan_native_byte_flip_dispatch(
            1,
            1,
            3,
            65_535,
            u32::MAX,
            u64::MAX,
            super::GPU_BUFFER_CAPACITY,
        )
        .is_none()
    );
    assert!(
        super::plan_native_byte_flip_dispatch(
            1,
            1,
            1,
            65_535,
            3,
            u64::MAX,
            super::GPU_BUFFER_CAPACITY,
        )
        .is_none()
    );
    assert!(
        super::plan_native_byte_flip_dispatch(
            1,
            1,
            1,
            65_535,
            u32::MAX,
            3,
            super::GPU_BUFFER_CAPACITY,
        )
        .is_none()
    );
    assert!(
        super::plan_native_byte_flip_dispatch(1, 1, 1, 65_535, u32::MAX, u64::MAX, 0,).is_none()
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_flip_keeps_mode_specific_storage_and_pixels() {
    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error) if error.to_string().contains("GPU adapter not available") => return,
        Err(error) => panic!("native Flip GPU initialization failed: {error}"),
    }
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let cases = [
        (
            "1",
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(
                    5,
                    3,
                    vec![0, 255, 0, 255, 255, 255, 0, 0, 255, 0, 0, 0, 255, 255, 0],
                )
                .unwrap(),
            ),
            24,
        ),
        (
            "L",
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(5, 3, (0..15).map(|value| value * 17).collect()).unwrap(),
            ),
            24,
        ),
        (
            "P",
            DynamicImage::ImageLuma8(
                GrayImage::from_raw(5, 3, (0..15).map(|value| value * 13).collect()).unwrap(),
            ),
            24,
        ),
        (
            "LA",
            DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(
                    5,
                    3,
                    (0..30).map(|value: u8| value.wrapping_mul(7)).collect(),
                )
                .unwrap(),
            ),
            36,
        ),
        (
            "PA",
            DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(
                    5,
                    3,
                    (0..30).map(|value: u8| value.wrapping_mul(9)).collect(),
                )
                .unwrap(),
            ),
            36,
        ),
        (
            "RGB",
            DynamicImage::ImageRgb8(
                RgbImage::from_raw(
                    5,
                    3,
                    (0usize..45)
                        .map(|index| index.wrapping_mul(47).wrapping_add(index / 5) as u8)
                        .collect(),
                )
                .unwrap(),
            ),
            48,
        ),
        (
            "RGBA",
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(
                    5,
                    3,
                    (0usize..60)
                        .map(|index| index.wrapping_mul(31).wrapping_add(index / 7) as u8)
                        .collect(),
                )
                .unwrap(),
            ),
            60,
        ),
    ];
    assert_native_flip_cases(cases);
}

#[cfg(target_endian = "little")]
type NativeFlipCase = (&'static str, DynamicImage, u64);

#[cfg(target_endian = "little")]
fn assert_native_flip_cases(cases: [NativeFlipCase; 7]) {
    use crate::compute::{execute_prepared, prepare_execution, registry};

    let op = PipelineOp::Flip;
    let ops = [op.clone()];
    let prepared = prepare_execution(&ops, Some(Backend::Gpu)).unwrap();
    for (mode, source, expected_transfer_bytes) in cases {
        let expected = registry::execute_cpu(&op, &source, Some(mode)).unwrap();
        let actual = execute_prepared(&prepared, &ops, &source, Some(mode)).unwrap();
        assert_eq!(actual.dimensions(), expected.dimensions(), "mode {mode}");
        assert_eq!(actual.color(), expected.color(), "mode {mode}");
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "mode {mode}");
        let receipt = Backend::take_pipeline_telemetry()
            .unwrap_or_else(|| panic!("native GPU Flip receipt missing for mode {mode}"));
        assert_eq!(receipt.0, Some(Backend::Gpu), "mode {mode}");
        assert_eq!(receipt.1, Backend::Gpu, "mode {mode}");
        assert_eq!(receipt.2, 1, "mode {mode}");
        assert_eq!(receipt.6, Some(1), "mode {mode}");
        assert_eq!(receipt.7, None, "mode {mode}");
        let resources = receipt
            .8
            .unwrap_or_else(|| panic!("native GPU Flip transfer counters missing for mode {mode}"));
        assert_eq!(
            resources.upload_bytes, expected_transfer_bytes,
            "mode {mode}"
        );
        assert_eq!(
            resources.readback_bytes, expected_transfer_bytes,
            "mode {mode}"
        );
        assert_eq!(resources.host_allocation_count, 1, "mode {mode}");
        assert_eq!(
            resources.host_allocated_bytes,
            u64::try_from(source.as_bytes().len()).unwrap(),
            "mode {mode}"
        );
        assert_eq!(resources.mode_conversion_count, 0, "mode {mode}");
    }
}
