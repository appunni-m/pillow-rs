use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_mapped_input_capacity_and_unmapped_reuse() {
    let Some(gpu) = primary_readback_test_gpu() else {
        return;
    };
    let mut buffers = super::BufferPool::new(&gpu.device, 15, true);
    let base_bytes = buffers.retained_bytes();
    for (width, height, retained_bytes) in [(1, 1, 4), (3, 5, 48), (1, 1, 48)] {
        let rgb = RgbImage::from_raw(
            width,
            height,
            vec![37; width as usize * height as usize * 3],
        )
        .unwrap();
        let layout = super::packed_rgb_transpose_layout(width, height, 1).unwrap();
        gpu.upload_packed_rgb_mapped(&rgb, &layout, &mut buffers)
            .unwrap();
        assert_eq!(buffers.retained_bytes(), base_bytes + retained_bytes);
        let input = &buffers.mapped_native_input.as_ref().unwrap().buffer;
        assert!(input.usage().contains(wgpu::BufferUsages::MAP_WRITE));
        assert!(!input.usage().contains(wgpu::BufferUsages::MAP_READ));
        assert!(
            !buffers
                .buf_a
                .usage()
                .contains(wgpu::BufferUsages::MAP_WRITE)
        );
        assert!(
            !buffers
                .buf_b
                .usage()
                .contains(wgpu::BufferUsages::MAP_WRITE)
        );
        assert!(buffers.buf_b.usage().contains(wgpu::BufferUsages::MAP_READ));
        // Re-mapping would fail if upload left a host view or mapping
        // alive. No shader or submission is needed for this idle buffer.
        gpu.wait_for_buffer_mapping(
            layout.transfer_bytes,
            input,
            wgpu::MapMode::Write,
            "GPU mapped input reuse test",
        )
        .unwrap();
        input.unmap();
    }
    let retained = buffers.retained_bytes();
    let rgb = RgbImage::from_raw(1, 1, vec![1, 2, 3]).unwrap();
    let malformed = super::PackedRgbTransposeLayout {
        native_bytes: 3,
        transfer_bytes: 0,
        workgroups: 1,
    };
    assert!(
        gpu.upload_packed_rgb_mapped(&rgb, &malformed, &mut buffers)
            .is_err()
    );
    assert_eq!(buffers.retained_bytes(), retained);
    let layout = super::packed_rgb_transpose_layout(1, 1, 1).unwrap();
    gpu.upload_packed_rgb_mapped(&rgb, &layout, &mut buffers)
        .unwrap();
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_upload_paths_concurrent_reuse_and_resources() {
    gpu_native_upload_paths_concurrent_reuse_and_resources(3);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_rgba_upload_paths_concurrent_reuse_and_resources() {
    gpu_native_upload_paths_concurrent_reuse_and_resources(4);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_transpose_public_fusion_and_inferred_mode() {
    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error) if error.to_string().contains("GPU adapter not available") => return,
        Err(error) => panic!("packed RGB GPU initialization failed: {error}"),
    }
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let bytes: Vec<u8> = (0usize..5 * 3 * 3)
        .map(|index| index.wrapping_mul(47).wrapping_add(index / 5) as u8)
        .collect();
    let source = DynamicImage::ImageRgb8(RgbImage::from_raw(5, 3, bytes).unwrap());
    assert_packed_rgb_transpose_fusion(&source);
    assert_packed_rgb_flip_backend(&source);
}

#[cfg(target_endian = "little")]
fn assert_packed_rgb_transpose_fusion(source: &DynamicImage) {
    use crate::compute::{execute_prepared, prepare_execution, registry};
    use crate::pipeline::TransposeMethod;

    let ops = [
        PipelineOp::Transpose {
            method: TransposeMethod::FlipTopBottom,
        },
        PipelineOp::Transpose {
            method: TransposeMethod::Rotate90,
        },
    ];
    for mode in [None, Some("RGB")] {
        let expected = ops.iter().fold(source.clone(), |image, op| {
            registry::execute_cpu(op, &image, mode).unwrap()
        });
        let prepared = prepare_execution(&ops, Some(Backend::Gpu)).unwrap();
        let actual = execute_prepared(&prepared, &ops, source, mode).unwrap();
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.color(), expected.color());
        assert_eq!(actual.as_bytes(), expected.as_bytes());
        let receipt = Backend::take_pipeline_telemetry().expect("fused RGB GPU receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, 2);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.expect("fused RGB transfer counters");
        assert_eq!(resources.upload_bytes, 48);
        assert_eq!(resources.readback_bytes, 48);
        assert_eq!(resources.host_allocation_count, 1);
        assert_eq!(resources.host_allocated_bytes, 45);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[cfg(target_endian = "little")]
fn assert_packed_rgb_flip_backend(source: &DynamicImage) {
    use crate::compute::{execute_prepared, prepare_execution, registry};

    let flip_ops = [PipelineOp::Flip];
    for mode in [None, Some("RGB")] {
        let expected = registry::execute_cpu(&PipelineOp::Flip, source, mode).unwrap();
        let prepared = prepare_execution(&flip_ops, Some(Backend::Gpu)).unwrap();
        let actual = execute_prepared(&prepared, &flip_ops, source, mode).unwrap();
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.color(), expected.color());
        assert_eq!(actual.as_bytes(), expected.as_bytes());
        let receipt = Backend::take_pipeline_telemetry().expect("native RGB Flip receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, 1);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.expect("native RGB Flip transfer counters");
        assert_eq!(resources.upload_bytes, 48);
        assert_eq!(resources.readback_bytes, 48);
        assert_eq!(resources.host_allocation_count, 1);
        assert_eq!(resources.host_allocated_bytes, 45);
        assert_eq!(resources.mode_conversion_count, 0);
    }
}
