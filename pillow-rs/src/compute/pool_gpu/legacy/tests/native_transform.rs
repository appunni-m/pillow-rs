use super::*;

struct RestoreTransformDiagnostics {
    telemetry: bool,
    shader_coverage: bool,
}

impl Drop for RestoreTransformDiagnostics {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.telemetry);
        Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
    }
}

fn native_transform_gpu_available(context: &str) -> bool {
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
        Err(error) => panic!("native GPU {context} initialization failed: {error}"),
    }
}

fn enable_native_transform_diagnostics() -> RestoreTransformDiagnostics {
    let restore = RestoreTransformDiagnostics {
        telemetry: Backend::set_pipeline_telemetry_enabled(true),
        shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
    };
    let _ = Backend::take_gpu_shader_coverage();
    restore
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_luma_affine_nearest_transform_reads_and_writes_packed_bytes() {
    struct RestoreDiagnostics {
        telemetry: bool,
        shader_coverage: bool,
    }
    impl Drop for RestoreDiagnostics {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.telemetry);
            Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
        }
    }

    let _restore = RestoreDiagnostics {
        telemetry: Backend::set_pipeline_telemetry_enabled(true),
        shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
    };
    let _ = Backend::take_gpu_shader_coverage();
    let source_bytes = (0..15)
        .map(|index| (index * 37 + 11) as u8)
        .collect::<Vec<_>>();
    let source = Image::frombytes("L", (5, 3), &source_bytes).expect("odd L source");
    let transformed = source
        .transform_public(
            (5, 3),
            0,
            Some(TransformData::Affine(vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0])),
            0,
            0,
            Some(TransformFill::Scalar(173)),
        )
        .expect("odd affine nearest L transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU odd L transform");
    let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("native GPU odd L transform failed: {error}"),
    };
    assert_eq!(actual, expected, "native L transform parity");

    assert_native_luma_transform_receipt();
    assert_aligned_native_luma_transform();
}

#[cfg(target_endian = "little")]
fn assert_native_luma_transform_receipt() {
    let receipt = Backend::take_pipeline_telemetry()
        .expect("native L transform must publish a telemetry receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let shader_coverage = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_coverage.len(), 1);
    assert_eq!(
        shader_coverage[0].variant_name,
        "__internal_transform_l_nearest_packed"
    );
    assert_eq!(
        shader_coverage[0].shader_file,
        "transform_l_nearest_packed.wgsl"
    );
    let resources = receipt.8.expect("native L transform resources");
    assert_eq!(resources.upload_bytes, 16);
    assert_eq!(resources.readback_bytes, 16);
    assert_eq!(resources.mode_conversion_count, 0);
}

#[cfg(target_endian = "little")]
fn assert_aligned_native_luma_transform() {
    // The common even-width path keeps every packed four-byte output word
    // inside one row and advances fixed coordinates from a shared base.
    let aligned_bytes = (0..32)
        .map(|index| (index * 29 + 7) as u8)
        .collect::<Vec<_>>();
    let aligned_source = Image::frombytes("L", (8, 4), &aligned_bytes).expect("aligned L source");
    let aligned_transform = aligned_source
        .transform_public(
            (8, 4),
            0,
            Some(TransformData::Affine(vec![
                0.87, 0.21, -1.25, -0.16, 1.13, 0.5,
            ])),
            0,
            0,
            Some(TransformFill::Scalar(173)),
        )
        .expect("aligned affine nearest L transform");
    let aligned_expected = aligned_transform
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU aligned L transform");
    let aligned_actual = aligned_transform
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("GPU aligned L transform");
    assert_eq!(aligned_actual, aligned_expected, "aligned native L parity");

    let aligned_receipt =
        Backend::take_pipeline_telemetry().expect("aligned native L transform telemetry");
    assert_eq!(aligned_receipt.0, Some(Backend::Gpu));
    assert_eq!(aligned_receipt.1, Backend::Gpu);
    assert_eq!(aligned_receipt.6, Some(1));
    assert_eq!(aligned_receipt.7, None);
    let aligned_coverage = Backend::take_gpu_shader_coverage();
    assert_eq!(aligned_coverage.len(), 1);
    assert_eq!(
        aligned_coverage[0].variant_name,
        "__internal_transform_l_nearest_packed"
    );
    let aligned_resources = aligned_receipt.8.expect("aligned native L resources");
    assert_eq!(aligned_resources.upload_bytes, 32);
    assert_eq!(aligned_resources.readback_bytes, 32);
    assert_eq!(aligned_resources.mode_conversion_count, 0);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_rgb_affine_nearest_transform_reads_and_writes_packed_bytes() {
    if !native_transform_gpu_available("RGB Transform") {
        return;
    }
    let _restore = enable_native_transform_diagnostics();
    for (width, height) in [(5u32, 3u32), (8u32, 4u32)] {
        assert_native_rgb_affine_case(width, height);
    }
}

#[cfg(target_endian = "little")]
fn assert_native_rgb_affine_case(width: u32, height: u32) {
    let source_bytes = (0..(width * height * 3) as usize)
        .map(|index| (index * 29 + 7) as u8)
        .collect::<Vec<_>>();
    let source =
        Image::frombytes("RGB", (width, height), &source_bytes).expect("native RGB source");
    let transformed = source
        .transform_public(
            (width, height),
            0,
            Some(TransformData::Affine(vec![
                0.87, 0.21, -1.25, -0.16, 1.13, 0.5,
            ])),
            0,
            0,
            Some(TransformFill::Components(vec![199, 71, 17])),
        )
        .expect("native affine nearest RGB transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU native RGB Transform");
    let _ = Backend::take_pipeline_telemetry();
    let _ = Backend::take_gpu_shader_coverage();
    let actual = transformed
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("GPU native RGB Transform");
    assert_eq!(actual, expected, "native RGB {width}x{height} parity");

    let receipt =
        Backend::take_pipeline_telemetry().expect("native RGB Transform must publish telemetry");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native RGB Transform resources");
    let transfer_bytes = (u64::from(width) * u64::from(height) * 3).div_ceil(4) * 4;
    assert_eq!(resources.upload_bytes, transfer_bytes);
    assert_eq!(resources.readback_bytes, transfer_bytes);
    assert_eq!(resources.mode_conversion_count, 0);

    let shader_coverage = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_coverage.len(), 1);
    assert_eq!(
        shader_coverage[0].variant_name,
        "__internal_transform_rgb_nearest_packed"
    );
    assert_eq!(
        shader_coverage[0].shader_file,
        "transform_rgb_nearest_packed.wgsl"
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_la_affine_nearest_transform_keeps_two_channel_bytes() {
    if !native_transform_gpu_available("LA Transform") {
        return;
    }
    let _restore = enable_native_transform_diagnostics();
    for (width, height) in [(5u32, 3u32), (8u32, 4u32), (4096u32, 4096u32)] {
        assert_native_la_affine_case(width, height);
    }
}

#[cfg(target_endian = "little")]
fn assert_native_la_affine_case(width: u32, height: u32) {
    let source_bytes = (0..(width * height * 2) as usize)
        .map(|index| (index * 29 + 7) as u8)
        .collect::<Vec<_>>();
    let source = Image::frombytes("LA", (width, height), &source_bytes).expect("native LA source");
    let transformed = source
        .transform_public(
            (width, height),
            0,
            Some(TransformData::Affine(vec![
                0.87, 0.21, -1.25, -0.16, 1.13, 0.5,
            ])),
            0,
            0,
            Some(TransformFill::Components(vec![173, 127])),
        )
        .expect("native affine nearest LA transform");
    let expected = transformed
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU native LA Transform");
    let _ = Backend::take_pipeline_telemetry();
    let _ = Backend::take_gpu_shader_coverage();
    let actual = transformed
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("GPU native LA Transform");
    assert_eq!(actual, expected, "native LA {width}x{height} parity");

    let receipt = Backend::take_pipeline_telemetry()
        .expect("native LA Transform must publish a telemetry receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native LA Transform resources");
    let transfer_bytes = (u64::from(width) * u64::from(height) * 2).div_ceil(4) * 4;
    assert_eq!(resources.upload_bytes, transfer_bytes);
    assert_eq!(resources.readback_bytes, transfer_bytes);
    assert_eq!(resources.mode_conversion_count, 0);

    let shader_coverage = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_coverage.len(), 1);
    assert_eq!(
        shader_coverage[0].variant_name,
        "__internal_transform_la_nearest_packed"
    );
    assert_eq!(
        shader_coverage[0].shader_file,
        "transform_la_nearest_packed.wgsl"
    );
}
