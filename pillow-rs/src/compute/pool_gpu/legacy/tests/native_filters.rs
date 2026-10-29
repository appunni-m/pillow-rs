use super::*;

struct RestorePipelineTelemetry(bool);

impl Drop for RestorePipelineTelemetry {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.0);
    }
}

fn take_native_filter_resources(
    receipt_message: &str,
    resources_message: &str,
    expected_dispatch_count: u64,
) -> crate::compute::PipelineResourceTelemetry {
    let receipt = Backend::take_pipeline_telemetry().expect(receipt_message);
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(expected_dispatch_count));
    assert_eq!(receipt.7, None);
    receipt.8.expect(resources_message)
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_luma_box_blur_radius_one_matches_cpu_at_edges_and_fuses_axes() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let operation = PipelineOp::BoxBlur { radius: 1 };
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (5, 3), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let pixels = (0..pixel_count)
            .map(|index| ((index * 71 + index / 9 * 37 + 23) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, pixels).expect("L BoxBlur source"),
        );
        let expected = crate::compute::registry::execute_cpu(&operation, &source, Some("L"))
            .expect("CPU L BoxBlur reference");
        let prepared = prepare_execution(std::slice::from_ref(&operation), Some(Backend::Gpu))
            .expect("GPU L BoxBlur routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(
                &prepared,
                std::slice::from_ref(&operation),
                &source,
                Some("L"),
            ),
            "native GPU L BoxBlur failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLuma8(_)));
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native L blur receipt", "native L blur resources", 1);
        let transfer_bytes = pixel_count.div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_rgb_box_blur_radius_one_matches_cpu_at_edges_and_fuses_axes() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let operation = PipelineOp::BoxBlur { radius: 1 };
    for (width, height) in [
        (1u32, 1u32),
        (1, 3),
        (4, 3),
        (5, 3),
        (33, 35),
        (65, 47),
        (67, 53),
    ] {
        let byte_count = width as usize * height as usize * 3;
        let pixels = (0..byte_count)
            .map(|index| ((index * 71 + index / 9 * 37 + 23) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, pixels).expect("RGB BoxBlur source"),
        );
        let expected = crate::compute::registry::execute_cpu(&operation, &source, Some("RGB"))
            .expect("CPU RGB BoxBlur reference");
        let prepared = prepare_execution(std::slice::from_ref(&operation), Some(Backend::Gpu))
            .expect("GPU RGB BoxBlur routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(
                &prepared,
                std::slice::from_ref(&operation),
                &source,
                Some("RGB"),
            ),
            "native GPU RGB BoxBlur failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageRgb8(_)));
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native RGB blur receipt", "native RGB blur resources", 1);
        let transfer_bytes = byte_count.div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
fn packed_la_dispatch_respects_word_and_adapter_boundaries() {
    assert_eq!(plan_packed_la_dispatch(1, 1, 65_535).unwrap(), (1, 1));
    assert_eq!(
        plan_packed_la_dispatch(1024, 768, 65_535).unwrap(),
        (1536, 1)
    );
    assert_eq!(
        plan_packed_la_dispatch(4096, 4096, 65_535).unwrap(),
        (32_768, 1)
    );
    assert_eq!(plan_packed_la_dispatch(1024, 1, 2).unwrap(), (2, 1));
    assert!(plan_packed_la_dispatch(1025, 1, 2).is_err());
    assert!(plan_packed_la_dispatch(0, 1, 65_535).is_err());
    assert!(plan_packed_la_dispatch(1, 0, 65_535).is_err());
    assert!(plan_packed_la_dispatch(1, 1, 0).is_err());
    assert!(plan_packed_la_dispatch(u32::MAX, 2, 65_535).is_err());
}

#[test]
fn packed_la_rank_filter_work_estimate_respects_selection_cost() {
    assert_eq!(super::gpu_packed_la_rank_filter_3_work_items((1, 1), 0), 18);
    assert_eq!(super::gpu_packed_la_rank_filter_3_work_items((1, 1), 1), 46);
    assert_eq!(
        super::gpu_packed_la_rank_filter_3_work_items((1, 1), 4),
        162
    );
    assert_eq!(super::gpu_packed_la_rank_filter_3_work_items((1, 1), 8), 18);
    assert!(
        super::gpu_packed_la_rank_filter_3_work_items((4096, 4096), 1)
            <= super::MAX_GPU_SHADER_WORK_ITEMS
    );
    assert!(
        super::gpu_packed_la_rank_filter_3_work_items((4096, 4096), 4)
            > super::MAX_GPU_SHADER_WORK_ITEMS
    );

    let max_rank_one_pixels = super::MAX_GPU_SHADER_WORK_ITEMS / 46;
    assert!(
        super::gpu_packed_la_rank_filter_3_work_items((max_rank_one_pixels as u32, 1), 1)
            <= super::MAX_GPU_SHADER_WORK_ITEMS
    );
    assert!(
        super::gpu_packed_la_rank_filter_3_work_items(((max_rank_one_pixels + 1) as u32, 1), 1)
            > super::MAX_GPU_SHADER_WORK_ITEMS
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_filter_5x5_preserves_channels_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
    let kernel = std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
    let op = [PipelineOp::Filter5x5 {
        kernel,
        scale: 256.0,
        offset: 0.0,
    }];

    for (width, height) in [(1u32, 1u32), (7, 5), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count * 2)
            .map(|index| ((index * 73 + index / 5 * 29 + 17) & 0xff) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("LA filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
            .expect("CPU LA Filter5x5 reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA Filter5x5 routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("LA")),
            "native GPU LA Filter5x5 failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources = take_native_filter_resources(
            "native LA Filter5x5 execution receipt",
            "native LA Filter5x5 resources",
            1,
        );
        let transfer_bytes = (pixel_count * 2).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_binomial_filter_3x3_preserves_channels_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::Filter3x3 {
        kernel: [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0],
        scale: 16.0,
        offset: 0.0,
    }];

    for (width, height) in [(1u32, 1u32), (7, 5), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count * 2)
            .map(|index| ((index * 73 + index / (width as usize * 2) * 29 + 17) & 0xff) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("LA filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
            .expect("CPU LA Filter3x3 reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA Filter3x3 routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("LA")),
            "native GPU LA Filter3x3 failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources = take_native_filter_resources(
            "native LA Filter3x3 execution receipt",
            "native LA Filter3x3 resources",
            1,
        );
        let transfer_bytes = (pixel_count * 2).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_median_filter_preserves_odd_width_rows_and_alpha() {
    use crate::compute::{execute_prepared, prepare_execution};

    let (width, height) = (65u32, 47u32);
    let bytes = (0..width as usize * height as usize * 2)
        .map(|index| ((index * 71 + index / 11 * 29 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let source = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(width, height, bytes).expect("odd-width LA source"),
    );
    let op = [PipelineOp::MedianFilter { size: 3 }];
    let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
        .expect("CPU LA MedianFilter reference");
    let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA MedianFilter routing");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let Some(actual) = native_gpu_result_or_skip(
        execute_prepared(&prepared, &op, &source, Some("LA")),
        "native GPU LA MedianFilter failed",
    ) else {
        Backend::set_pipeline_telemetry_enabled(previous);
        return;
    };
    assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let resources =
        take_native_filter_resources("native LA median receipt", "native LA median resources", 1);
    let transfer_bytes = (width as usize * height as usize * 2).div_ceil(4) * 4;
    assert_eq!(resources.upload_bytes, transfer_bytes as u64);
    assert_eq!(resources.readback_bytes, transfer_bytes as u64);
    assert_eq!(resources.mode_conversion_count, 0);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_rank_filter_preserves_rank_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let (width, height) = (65u32, 47u32);
    let bytes = (0..width as usize * height as usize * 2)
        .map(|index| ((index * 71 + index / 11 * 29 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let source = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(width, height, bytes).expect("odd-width LA source"),
    );

    for rank in [0, 1, 4, 8] {
        let op = [PipelineOp::RankFilter { size: 3, rank }];
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
            .expect("CPU LA RankFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA RankFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("LA")),
            format_args!("native GPU LA RankFilter(rank={rank}) failed"),
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "rank={rank}");

        let resources = take_native_filter_resources(
            "native LA RankFilter execution receipt",
            "native LA RankFilter resources",
            1,
        );
        let transfer_bytes = (width as usize * height as usize * 2).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_max_filter_preserves_edges_alpha_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MaxFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (5, 3), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count * 2)
            .map(|index| ((index * 71 + index / 11 * 29 + 17) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("LA max-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
            .expect("CPU LA MaxFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA MaxFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("LA")),
            "native GPU LA MaxFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native LA max receipt", "native LA max resources", 1);
        let transfer_bytes = (pixel_count * 2).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_min_filter_preserves_edges_alpha_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MinFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (5, 3), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count * 2)
            .map(|index| ((index * 79 + index / 9 * 23 + 31) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, bytes).expect("LA min-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
            .expect("CPU LA MinFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA MinFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("LA")),
            "native GPU LA MinFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native LA min receipt", "native LA min resources", 1);
        let transfer_bytes = (pixel_count * 2).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_median_filter_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MedianFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (33, 35)] {
        let bytes = (0..width as usize * height as usize * 3)
            .map(|index| ((index * 73 + index / 13 * 31 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, bytes).expect("RGB median source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("RGB"))
            .expect("CPU RGB MedianFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU RGB MedianFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("RGB")),
            "native GPU RGB MedianFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageRgb8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources = take_native_filter_resources(
            "native RGB median receipt",
            "native RGB median resources",
            1,
        );
        let transfer_bytes = (width as usize * height as usize * 3).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_rank_one_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::RankFilter { size: 3, rank: 1 }];
    for (width, height) in [(33u32, 35u32), (65, 47)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count)
            .map(|index| ((index * 37 + index / width as usize * 11 + 3) % 5) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, bytes).expect("L rank-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("L"))
            .expect("CPU L RankFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU L RankFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("L")),
            "native GPU L RankFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLuma8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native L rank receipt", "native L rank resources", 1);
        let transfer_bytes = pixel_count.div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_max_filter_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MaxFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (5, 3), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count)
            .map(|index| ((index * 73 + index / 13 * 31 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, bytes).expect("L max-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("L"))
            .expect("CPU L MaxFilter reference");
        let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU L MaxFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("L")),
            "native GPU L MaxFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLuma8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native L max receipt", "native L max resources", 1);
        let transfer_bytes = pixel_count.div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_min_filter_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MinFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (5, 3), (33, 35)] {
        let pixel_count = width as usize * height as usize;
        let bytes = (0..pixel_count)
            .map(|index| ((index * 73 + index / 13 * 31 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, bytes).expect("L min-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("L"))
            .expect("CPU L MinFilter reference");
        let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU L MinFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("L")),
            "native GPU L MinFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageLuma8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native L min receipt", "native L min resources", 1);
        let transfer_bytes = pixel_count.div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_max_filter_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MaxFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (8, 3), (5, 3), (33, 35)] {
        let bytes = (0..width as usize * height as usize * 3)
            .map(|index| ((index * 73 + index / 13 * 31 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, bytes).expect("RGB max-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("RGB"))
            .expect("CPU RGB MaxFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU RGB MaxFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("RGB")),
            "native GPU RGB MaxFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageRgb8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native RGB max receipt", "native RGB max resources", 1);
        let transfer_bytes = (width as usize * height as usize * 3).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_min_filter_preserves_edges_and_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestorePipelineTelemetry(previous);
    let op = [PipelineOp::MinFilter { size: 3 }];
    for (width, height) in [(1u32, 1u32), (1, 3), (4, 3), (8, 3), (5, 3), (33, 35)] {
        let bytes = (0..width as usize * height as usize * 3)
            .map(|index| ((index * 73 + index / 13 * 31 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, bytes).expect("RGB min-filter source"),
        );
        let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("RGB"))
            .expect("CPU RGB MinFilter reference");
        let prepared =
            prepare_execution(&op, Some(Backend::Gpu)).expect("GPU RGB MinFilter routing");
        let Some(actual) = native_gpu_result_or_skip(
            execute_prepared(&prepared, &op, &source, Some("RGB")),
            "native GPU RGB MinFilter failed",
        ) else {
            return;
        };
        assert!(matches!(&actual, DynamicImage::ImageRgb8(_)));
        assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");

        let resources =
            take_native_filter_resources("native RGB min receipt", "native RGB min resources", 1);
        let transfer_bytes = (width as usize * height as usize * 3).div_ceil(4) * 4;
        assert_eq!(resources.upload_bytes, transfer_bytes as u64);
        assert_eq!(resources.readback_bytes, transfer_bytes as u64);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_gaussian_blur_preserves_odd_width_rows_and_alpha() {
    use crate::compute::{execute_prepared, prepare_execution};

    let (width, height) = (65u32, 47u32);
    let bytes = (0..width as usize * height as usize * 2)
        .map(|index| ((index * 71 + index / 11 * 29 + 17) % 256) as u8)
        .collect::<Vec<_>>();
    let source = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(width, height, bytes).expect("odd-width LA source"),
    );
    let op = [PipelineOp::GaussianBlur { sigma: 2.0 }];
    let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
        .expect("CPU LA GaussianBlur reference");
    let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA GaussianBlur routing");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let Some(actual) = native_gpu_result_or_skip(
        execute_prepared(&prepared, &op, &source, Some("LA")),
        "native GPU LA GaussianBlur failed",
    ) else {
        Backend::set_pipeline_telemetry_enabled(previous);
        return;
    };
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let resources =
        take_native_filter_resources("native LA blur receipt", "native LA blur resources", 6);
    let transfer_bytes = (width as usize * height as usize * 2).div_ceil(4) * 4;
    assert_eq!(resources.upload_bytes, transfer_bytes as u64);
    assert_eq!(resources.readback_bytes, transfer_bytes as u64);
    assert_eq!(resources.mode_conversion_count, 0);
    Backend::set_pipeline_telemetry_enabled(previous);
}
