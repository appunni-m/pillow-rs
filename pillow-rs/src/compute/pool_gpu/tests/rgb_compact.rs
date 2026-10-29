use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_rgb_compact_input_requires_supported_native_rgb_operations() {
    let convert_rgba = PipelineOp::Convert {
        mode: ColorMode::RGBA,
        matrix: None,
        dither: None,
    };
    let rgb =
        DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![11, 37, 83, 149, 211, 251]).unwrap());
    let transform = PipelineOp::Transform {
        w: 2,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Bilinear,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };
    let equalize = PipelineOp::Equalize;
    let median = PipelineOp::MedianFilter { size: 3 };
    let min_filter = PipelineOp::MinFilter { size: 3 };
    let max_filter = PipelineOp::MaxFilter { size: 3 };
    let box_blur = PipelineOp::BoxBlur { radius: 1 };
    assert_rgb_compact_conversion_and_transform_support(&rgb, &convert_rgba, &transform);
    assert_rgb_compact_filter_support(
        &rgb,
        &equalize,
        &median,
        &min_filter,
        &max_filter,
        &box_blur,
    );
    assert_rgb_compact_size_rejections(&rgb);
    assert_rgb_compact_mode_and_context_rejections(&rgb, &convert_rgba, &transform, &equalize);
    let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
    assert!(!super::gpu_native_rgb_compact_input(
        std::slice::from_ref(&convert_rgba),
        &empty,
        Some("RGB"),
        false
    ));
    let eval = PipelineOp::Eval {
        lut: Arc::from(vec![17; 3 * 256]),
    };
    assert!(!super::gpu_native_rgb_compact_input(
        std::slice::from_ref(&eval),
        &rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(&eval),
        &rgb,
        Some("RGB"),
        true
    ));
}

fn assert_rgb_compact_conversion_and_transform_support(
    rgb: &DynamicImage,
    convert_rgba: &PipelineOp,
    transform: &PipelineOp,
) {
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(convert_rgba),
        rgb,
        None,
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(convert_rgba),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(transform),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(transform),
        rgb,
        None,
        false
    ));
}

fn assert_rgb_compact_filter_support(
    rgb: &DynamicImage,
    equalize: &PipelineOp,
    median: &PipelineOp,
    min_filter: &PipelineOp,
    max_filter: &PipelineOp,
    box_blur: &PipelineOp,
) {
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(equalize),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(equalize),
        rgb,
        None,
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(median),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(median),
        rgb,
        None,
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(min_filter),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(min_filter),
        rgb,
        None,
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(max_filter),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(max_filter),
        rgb,
        None,
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(box_blur),
        rgb,
        Some("RGB"),
        false
    ));
    assert!(super::gpu_native_rgb_compact_input(
        std::slice::from_ref(box_blur),
        rgb,
        None,
        false
    ));
}

fn assert_rgb_compact_size_rejections(rgb: &DynamicImage) {
    assert!(!super::gpu_native_rgb_compact_input(
        &[PipelineOp::BoxBlur { radius: 2 }],
        rgb,
        Some("RGB"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        &[PipelineOp::MaxFilter { size: 5 }],
        rgb,
        Some("RGB"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        &[PipelineOp::MinFilter { size: 5 }],
        rgb,
        Some("RGB"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        &[PipelineOp::MedianFilter { size: 5 }],
        rgb,
        Some("RGB"),
        false
    ));
}

fn assert_rgb_compact_mode_and_context_rejections(
    rgb: &DynamicImage,
    convert_rgba: &PipelineOp,
    transform: &PipelineOp,
    equalize: &PipelineOp,
) {
    assert!(!super::gpu_native_rgb_compact_input(
        std::slice::from_ref(equalize),
        rgb,
        Some("RGBA"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        std::slice::from_ref(convert_rgba),
        rgb,
        Some("HSV"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        &[convert_rgba.clone(), PipelineOp::Duplicate],
        rgb,
        Some("RGB"),
        false
    ));
    assert!(!super::gpu_native_rgb_compact_input(
        &[transform.clone(), PipelineOp::Duplicate],
        rgb,
        Some("RGB"),
        false
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn rgb_transform_consumes_compact_input_for_affine_and_geometry_shaders() {
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestoreTelemetry(previous);

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
        Err(error) => panic!("GPU RGB Transform initialization failed: {error}"),
    }

    let (width, height) = (5u32, 3u32);
    let pixel_count = width as usize * height as usize;
    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(
            width,
            height,
            (0..pixel_count * 3)
                .map(|index| index.wrapping_mul(47).wrapping_add(index / 5) as u8)
                .collect(),
        )
        .expect("RGB transform source"),
    );
    for (name, mode, op) in rgb_transform_cases(width, height) {
        assert_rgb_transform_case(name, mode, &op, &source, pixel_count);
    }
}

fn rgb_transform_cases(
    width: u32,
    height: u32,
) -> [(&'static str, Option<&'static str>, PipelineOp); 3] {
    [
        (
            "affine nearest",
            None,
            PipelineOp::Transform {
                w: width,
                h: height,
                method: TransformMethod::Affine,
                data: Arc::from(vec![1.0, 0.0, -0.25, 0.0, 1.0, 0.125]),
                filter: ResampleFilter::Nearest,
                fill: Some((7, 11, 13, 255)),
                fill_is_none: false,
                palette_fill: None,
            },
        ),
        (
            "affine bilinear",
            Some("RGB"),
            PipelineOp::Transform {
                w: width,
                h: height,
                method: TransformMethod::Affine,
                data: Arc::from(vec![0.9375, 0.0, 0.25, 0.0, 1.0, 0.125]),
                filter: ResampleFilter::Bilinear,
                fill: Some((7, 11, 13, 255)),
                fill_is_none: false,
                palette_fill: None,
            },
        ),
        (
            "perspective geometry table",
            Some("RGB"),
            PipelineOp::Transform {
                w: width,
                h: height,
                method: TransformMethod::Perspective,
                data: Arc::from(vec![1.0, 0.0, 0.25, 0.0, 1.0, 0.125, 0.03125, 0.0078125]),
                filter: ResampleFilter::Bilinear,
                fill: Some((7, 11, 13, 255)),
                fill_is_none: false,
                palette_fill: None,
            },
        ),
    ]
}

fn assert_rgb_transform_case(
    name: &str,
    mode: Option<&str>,
    op: &PipelineOp,
    source: &DynamicImage,
    pixel_count: usize,
) {
    use crate::compute::BackendImpl;

    if name == "perspective geometry table" {
        assert!(super::gpu_transform_uses_geometry_table(
            op,
            mode,
            source.dimensions()
        ));
    }
    let expected = crate::compute::registry::execute_cpu(op, source, mode)
        .unwrap_or_else(|error| panic!("CPU {name}: {error}"));
    let actual = super::GpuPool
        .execute_batch_strict(
            &crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Gpu),
std::slice::from_ref(op), source, mode)
        .unwrap_or_else(|error| panic!("GPU {name}: {error}"));
    assert_eq!(actual.color(), expected.color(), "{name} mode");
    assert_eq!(
        actual.dimensions(),
        expected.dimensions(),
        "{name} dimensions"
    );
    assert_eq!(actual.as_bytes(), expected.as_bytes(), "{name} pixels");

    let resources = crate::compute::take_pipeline_resource_telemetry()
        .unwrap_or_else(|| panic!("{name} resource receipt"));
    assert_eq!(
        resources.upload_bytes,
        (pixel_count * 3).div_ceil(4) as u64 * 4,
        "{name} upload"
    );
    assert_eq!(
        resources.readback_bytes,
        pixel_count as u64 * 4,
        "{name} readback"
    );
    assert_eq!(
        resources.mode_conversion_count, 0,
        "{name} input conversion"
    );
    assert_eq!(
        crate::compute::take_pipeline_dispatch_count(),
        Some(1),
        "{name} dispatches"
    );
    assert_eq!(
        crate::compute::take_pipeline_backend_override(),
        None,
        "{name} fallback"
    );
}

#[test]
#[cfg(target_endian = "little")]
fn rgb_autocontrast_packed_point_matches_cpu_for_every_rgb_tail() {
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestoreTelemetry(previous);

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
        Err(error) => panic!("GPU RGB AutoContrast initialization failed: {error}"),
    }

    for (width, height) in [(5u32, 1u32), (3, 2), (5, 3), (8, 1)] {
        assert_rgb_autocontrast_packed_case(width, height);
    }
}

#[cfg(target_endian = "little")]
fn assert_rgb_autocontrast_packed_case(width: u32, height: u32) {
    use crate::compute::BackendImpl;

    let pixel_count = width as usize * height as usize;
    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(
            width,
            height,
            (0..pixel_count * 3)
                .map(|index| index.wrapping_mul(47).wrapping_add(index / 3) as u8)
                .collect(),
        )
        .expect("RGB AutoContrast source"),
    );
    let op = PipelineOp::Autocontrast {
        cutoff: 0.0,
        mask: None,
    };
    let expected = crate::compute::registry::execute_cpu(&op, &source, Some("RGB"))
        .unwrap_or_else(|error| panic!("CPU RGB AutoContrast {width}x{height}: {error}"));
    let actual = super::GpuPool
        .execute_batch_strict(
            &crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Gpu),
std::slice::from_ref(&op), &source, Some("RGB"))
        .unwrap_or_else(|error| panic!("GPU RGB AutoContrast {width}x{height}: {error}"));
    assert_eq!(actual.color(), expected.color(), "{width}x{height} mode");
    assert_eq!(
        actual.as_bytes(),
        expected.as_bytes(),
        "{width}x{height} pixels"
    );

    let resources = crate::compute::take_pipeline_resource_telemetry()
        .unwrap_or_else(|| panic!("{width}x{height} resource receipt"));
    let transfer_bytes = (pixel_count * 3).div_ceil(4) * 4;
    assert_eq!(
        resources.upload_bytes, transfer_bytes as u64,
        "{width}x{height} upload"
    );
    assert_eq!(
        resources.readback_bytes, transfer_bytes as u64,
        "{width}x{height} readback"
    );
    assert_eq!(
        resources.mode_conversion_count, 0,
        "{width}x{height} conversion"
    );
    assert_eq!(
        crate::compute::take_pipeline_dispatch_count(),
        Some(1),
        "{width}x{height} dispatch"
    );
    assert_eq!(
        crate::compute::take_pipeline_backend_override(),
        None,
        "{width}x{height} fallback"
    );
}

#[test]
#[cfg(target_endian = "little")]
fn rgb_equalize_packed_histogram_matches_cpu_for_every_rgb_tail() {
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestoreTelemetry(previous);

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
        Err(error) => panic!("GPU RGB Equalize initialization failed: {error}"),
    }

    // Exercise every compact RGB word tail plus the row-tiled dispatch
    // layout. These counts exceed Pillow's zero-step equalize boundary.
    for (width, height) in [(257u32, 1u32), (258, 1), (259, 1), (260, 1), (16, 17)] {
        assert_rgb_equalize_packed_case(width, height);
    }
}

#[cfg(target_endian = "little")]
fn assert_rgb_equalize_packed_case(width: u32, height: u32) {
    use crate::compute::BackendImpl;

    let pixel_count = width as usize * height as usize;
    let bytes: Vec<u8> = (0..pixel_count)
        .flat_map(|pixel| {
            [
                pixel.wrapping_mul(73).wrapping_add(pixel / 17) as u8,
                pixel
                    .wrapping_mul(41)
                    .wrapping_add(pixel / 11)
                    .wrapping_add(19) as u8,
                pixel
                    .wrapping_mul(29)
                    .wrapping_add(pixel / 7)
                    .wrapping_add(37) as u8,
            ]
        })
        .collect();
    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, bytes).expect("RGB Equalize source"),
    );
    let op = PipelineOp::Equalize;
    let expected = crate::compute::registry::execute_cpu(&op, &source, Some("RGB"))
        .unwrap_or_else(|error| panic!("CPU RGB Equalize {width}x{height}: {error}"));
    let actual = super::GpuPool
        .execute_batch_strict(
            &crate::compute::ExecutionContext::for_backend(crate::compute::Backend::Gpu),
std::slice::from_ref(&op), &source, Some("RGB"))
        .unwrap_or_else(|error| panic!("GPU RGB Equalize {width}x{height}: {error}"));
    assert_eq!(actual.color(), expected.color(), "{width}x{height} mode");
    assert_eq!(
        actual.as_bytes(),
        expected.as_bytes(),
        "{width}x{height} pixels"
    );

    let resources = crate::compute::take_pipeline_resource_telemetry()
        .unwrap_or_else(|| panic!("{width}x{height} resource receipt"));
    let transfer_bytes = (pixel_count * 3).div_ceil(4) * 4;
    assert_eq!(
        resources.upload_bytes, transfer_bytes as u64,
        "{width}x{height} upload"
    );
    assert_eq!(
        resources.readback_bytes, transfer_bytes as u64,
        "{width}x{height} readback"
    );
    assert_eq!(
        resources.mode_conversion_count, 0,
        "{width}x{height} conversion"
    );
    assert_eq!(
        crate::compute::take_pipeline_dispatch_count(),
        Some(3),
        "{width}x{height} compute dispatches"
    );
    assert_eq!(
        crate::compute::take_pipeline_backend_override(),
        None,
        "{width}x{height} fallback"
    );
}
