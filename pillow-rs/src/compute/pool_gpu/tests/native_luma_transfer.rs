use super::*;

#[test]
fn colorize_gpu_packs_luma_upload_and_keeps_exact_rgb() {
    use crate::compute::BackendImpl;

    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("GPU Colorize initialization failed: {error}"),
    }

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _telemetry = RestoreTelemetry(previous);

    let (width, height) = (37, 29);
    let pixels = width as usize * height as usize;
    let source = DynamicImage::ImageLuma8(
        GrayImage::from_raw(
            width,
            height,
            (0..pixels)
                .map(|index| index.wrapping_mul(47).wrapping_add(index / 7) as u8)
                .collect(),
        )
        .expect("Colorize L source"),
    );
    let op = PipelineOp::Colorize {
        black: (0, 0, 0),
        white: (255, 255, 255),
        mid: None,
        blackpoint: 0,
        midpoint: 127,
        whitepoint: 255,
    };
    let expected = crate::compute::registry::execute_cpu(&op, &source, Some("L"))
        .expect("CPU Colorize reference");
    let actual = super::GpuPool
        .execute_batch_strict(
            &crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Gpu),
std::slice::from_ref(&op), &source, Some("L"))
        .expect("strict GPU Colorize");
    assert_eq!(actual.color(), expected.color());
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert_eq!(actual.as_bytes(), expected.as_bytes());

    let resources =
        crate::compute::take_pipeline_resource_telemetry().expect("GPU Colorize resource receipt");
    assert_eq!(resources.upload_bytes, pixels.div_ceil(4) as u64 * 4);
    assert_eq!(resources.readback_bytes, pixels as u64 * 4);
    assert_eq!(resources.mode_conversion_count, 1);
    assert_eq!(crate::compute::take_pipeline_backend_override(), None);
}

#[test]
fn constant_gpu_packs_flat_luma_and_reads_back_aligned_tail() {
    use crate::compute::BackendImpl;

    let gpu = match super::GpuPool::ensure_init() {
        Ok(gpu) => gpu,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("GPU Constant initialization failed: {error}"),
    };

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _telemetry = RestoreTelemetry(previous);

    for (width, height, value) in [(1, 1, 0), (3, 2, 173), (5, 3, 255), (7, 5, 41)] {
        let pixel_count = width as usize * height as usize;
        let input = (0..pixel_count * 4)
            .map(|index| index.wrapping_mul(53).wrapping_add(19) as u8)
            .collect();
        let source = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, input).expect("RGBA Constant source"),
        );
        let op = PipelineOp::Constant { value };
        let expected = crate::compute::registry::execute_cpu(&op, &source, Some("RGBA"))
            .expect("CPU Constant reference");
        let actual = super::GpuPool
            .execute_batch_strict(
            &crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Gpu),
std::slice::from_ref(&op), &source, Some("RGBA"))
            .expect("strict GPU Constant");

        assert_eq!(actual.color(), expected.color());
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.as_bytes(), expected.as_bytes());
        let resources = crate::compute::take_pipeline_resource_telemetry()
            .expect("GPU Constant resource receipt");
        assert_eq!(resources.upload_bytes, 0);
        assert_eq!(
            resources.readback_bytes,
            super::compact_luma8_transfer_bytes(width, height).unwrap()
        );
        assert_eq!(resources.mode_conversion_count, 0);
        assert_eq!(
            resources.full_frame_copy_count,
            u64::from(!gpu.direct_primary_readback)
        );
        assert_eq!(crate::compute::take_pipeline_dispatch_count(), Some(1));
        assert_eq!(crate::compute::take_pipeline_backend_override(), None);
    }
}
