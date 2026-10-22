use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_transpose_tiled_requires_aligned_swapped_rows() {
    use crate::pipeline::TransposeMethod;

    for method in [
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ] {
        for (width, height, expected) in [
            (4, 3, Some((1, 1))),
            (20, 17, Some((2, 2))),
            (772, 768, Some((49, 48))),
            (3, 4, None),
            (17, 20, None),
            (0, 4, None),
            (4, 0, None),
        ] {
            assert_eq!(
                super::packed_rgb_transpose_tiled_dispatch(&method, width, height, 64),
                expected,
            );
        }
        assert_eq!(
            super::packed_rgb_transpose_tiled_dispatch(&method, 20, 17, 1),
            None,
        );
        assert_eq!(
            super::packed_rgb_transpose_tiled_dispatch(&method, 4, 3, 0),
            None,
        );
    }
    for method in [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate180,
    ] {
        assert_eq!(
            super::packed_rgb_transpose_tiled_dispatch(&method, 16, 16, 64),
            None,
        );
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_mapped_rgba_transpose_admission_preserves_other_modes() {
    let image = DynamicImage::ImageRgba8(RgbaImage::new(3, 5));
    let op = PipelineOp::Transpose {
        method: crate::pipeline::TransposeMethod::Rotate90,
    };
    for mode in [None, Some("RGBA")] {
        assert!(super::mapped_rgba_transpose_is_supported(
            std::slice::from_ref(&op),
            &image,
            mode,
            true
        ));
        assert!(!super::mapped_rgba_transpose_is_supported(
            std::slice::from_ref(&op),
            &image,
            mode,
            false
        ));
    }
    for mode in [
        "RGBX", "RGBa", "CMYK", "I", "F", "RGB", "P", "PA", "L", "LA",
    ] {
        assert!(!super::mapped_rgba_transpose_is_supported(
            std::slice::from_ref(&op),
            &image,
            Some(mode),
            true
        ));
    }
    for ops in [
        vec![],
        vec![PipelineOp::Duplicate],
        vec![op.clone(), PipelineOp::Duplicate],
    ] {
        assert!(!super::mapped_rgba_transpose_is_supported(
            &ops,
            &image,
            Some("RGBA"),
            true
        ));
    }
    let rgb = DynamicImage::ImageRgb8(RgbImage::new(3, 5));
    assert!(!super::mapped_rgba_transpose_is_supported(
        &[op],
        &rgb,
        Some("RGBA"),
        true
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_rgba_transpose_native_methods_and_transport_receipts() {
    use crate::pipeline::TransposeMethod as Method;
    let gpu = match super::GpuPool::ensure_init() {
        Ok(gpu) => gpu,
        Err(error) if error.to_string().contains("GPU adapter not available") => return,
        Err(error) => panic!("native RGBA GPU initialization failed: {error}"),
    };
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    for direct in [false, true] {
        if direct && !gpu.direct_primary_readback {
            continue;
        }
        for (width, height) in [
            (1, 1),
            (1, 17),
            (17, 1),
            (3, 5),
            (4, 3),
            (17, 20),
            (515, 513),
            (772, 768),
        ] {
            let bytes: Vec<u8> = (0usize..width as usize * height as usize * 4)
                .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
                .collect();
            let rgba = RgbaImage::from_raw(width, height, bytes).unwrap();
            let mut buffers = super::BufferPool::new(&gpu.device, width * height, direct);
            for method in [
                Method::FlipLeftRight,
                Method::FlipTopBottom,
                Method::Rotate90,
                Method::Rotate180,
                Method::Rotate270,
                Method::Transpose,
                Method::Transverse,
            ] {
                let expected = rgba_transpose_reference(&rgba, &method);
                let op = PipelineOp::Transpose { method };
                for mapped_input in [false, true] {
                    if mapped_input && !gpu.direct_primary_readback {
                        continue;
                    }
                    let _ = crate::compute::take_pipeline_dispatch_count();
                    let _ = crate::compute::take_pipeline_resource_telemetry();
                    let _ = Backend::take_pipeline_allocation_telemetry();
                    let actual = gpu
                        .execute_rgba_transpose_with_upload(&op, &rgba, &mut buffers, mapped_input)
                        .unwrap();
                    assert_eq!(actual.dimensions(), expected.dimensions());
                    assert_eq!(actual.color(), crate::raster::ColorType::Rgba8);
                    assert_eq!(actual.as_bytes(), expected.as_raw());
                    assert_eq!(crate::compute::take_pipeline_dispatch_count(), Some(1));
                    let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
                    let bytes = u64::from(width) * u64::from(height) * 4;
                    assert_eq!(resources.upload_bytes, bytes);
                    assert_eq!(resources.readback_bytes, bytes);
                    assert_eq!(resources.parameter_bytes, 32);
                    assert_eq!(resources.mode_conversion_count, 0);
                    assert_eq!(
                        resources.full_frame_copy_count,
                        u64::from(!mapped_input) + u64::from(!direct)
                    );
                    assert_eq!(resources.retained_cache_bytes, buffers.retained_bytes());
                    let allocation = Backend::take_pipeline_allocation_telemetry();
                    assert_eq!(allocation.allocation_count, 1);
                    assert_eq!(allocation.allocated_bytes, bytes);
                }
            }
        }
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_rgba_transpose_public_fusion_and_inferred_mode() {
    use crate::compute::{execute_prepared, prepare_execution};
    use crate::pipeline::TransposeMethod as Method;
    let Some(_gpu) = primary_readback_test_gpu() else {
        return;
    };
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let bytes: Vec<u8> = (0usize..17 * 19 * 4)
        .map(|index| index.wrapping_mul(47).wrapping_add(index / 5) as u8)
        .collect();
    let rgba = RgbaImage::from_raw(17, 19, bytes).unwrap();
    let expected = rgba_transpose_reference(&rgba, &Method::Transverse);
    let source = DynamicImage::ImageRgba8(rgba);
    let ops = [
        PipelineOp::Transpose {
            method: Method::FlipTopBottom,
        },
        PipelineOp::Transpose {
            method: Method::Rotate90,
        },
    ];
    for mode in [None, Some("RGBA")] {
        let prepared = prepare_execution(&ops, Some(Backend::Gpu)).unwrap();
        let actual = execute_prepared(&prepared, &ops, &source, mode).unwrap();
        assert_eq!(actual.dimensions(), expected.dimensions());
        assert_eq!(actual.as_bytes(), expected.as_raw());
        let receipt = Backend::take_pipeline_telemetry().unwrap();
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.2, 2);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.unwrap();
        assert_eq!(resources.upload_bytes, 17 * 19 * 4);
        assert_eq!(resources.readback_bytes, 17 * 19 * 4);
        assert_eq!(resources.host_allocation_count, 1);
        assert_eq!(resources.host_allocated_bytes, 17 * 19 * 4);
        assert_eq!(resources.mode_conversion_count, 0);
        assert_eq!(resources.full_frame_copy_count, 0);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_transpose_native_tails_and_methods() {
    use crate::compute::registry;
    use crate::pipeline::TransposeMethod;

    let gpu = match super::GpuPool::ensure_init() {
        Ok(gpu) => gpu,
        Err(error) if error.to_string().contains("GPU adapter not available") => return,
        Err(error) => panic!("packed RGB GPU initialization failed: {error}"),
    };
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let methods = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    for direct in [false, true] {
        if direct && !gpu.direct_primary_readback {
            continue;
        }
        for (width, height) in [
            (1, 1),
            (1, 2),
            (1, 3),
            (1, 4),
            (2, 1),
            (3, 1),
            (4, 1),
            (3, 5),
            (3, 4),
            (4, 3),
            (5, 3),
            (17, 15),
            (16, 17),
            (17, 20),
            (31, 28),
            (513, 515),
            (768, 772),
        ] {
            let bytes: Vec<u8> = (0usize..width as usize * height as usize * 3)
                .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
                .collect();
            let rgb = RgbImage::from_raw(width, height, bytes).unwrap();
            let source = DynamicImage::ImageRgb8(rgb.clone());
            let mut buffers = super::BufferPool::new(&gpu.device, width * height, direct);
            let layout = super::packed_rgb_transpose_layout(
                width,
                height,
                gpu.device.limits().max_compute_workgroups_per_dimension,
            )
            .unwrap();
            for method in &methods {
                let op = PipelineOp::Transpose {
                    method: method.clone(),
                };
                let expected = registry::execute_cpu(&op, &source, Some("RGB")).unwrap();
                for mapped_input in [false, true] {
                    if mapped_input && !gpu.direct_primary_readback {
                        continue;
                    }
                    let _ = crate::compute::take_pipeline_dispatch_count();
                    let _ = crate::compute::take_pipeline_resource_telemetry();
                    let _ = Backend::take_pipeline_allocation_telemetry();
                    let actual = gpu
                        .execute_packed_rgb_transpose_with_upload(
                            &op,
                            &rgb,
                            &layout,
                            &mut buffers,
                            mapped_input,
                        )
                        .unwrap();
                    assert_eq!(actual.color(), expected.color());
                    assert_eq!(actual.dimensions(), expected.dimensions());
                    assert_eq!(
                        actual.as_bytes(),
                        expected.as_bytes(),
                        "{width}x{height} {method:?} direct={direct} mapped={mapped_input}"
                    );
                    assert_eq!(crate::compute::take_pipeline_dispatch_count(), Some(1));
                    let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
                    assert_eq!(resources.upload_bytes, layout.transfer_bytes);
                    assert_eq!(resources.readback_bytes, layout.transfer_bytes);
                    assert_eq!(resources.mode_conversion_count, 0);
                    assert_eq!(
                        resources.full_frame_copy_count,
                        u64::from(!mapped_input) + u64::from(!direct),
                    );
                    assert_eq!(resources.retained_cache_bytes, buffers.retained_bytes());
                    let allocations = Backend::take_pipeline_allocation_telemetry();
                    assert_eq!(allocations.allocation_count, 1);
                    assert_eq!(allocations.allocated_bytes, layout.native_bytes as u64);
                }
            }
        }
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_rgb_hsv_transpose_tiled_public_pixels_and_receipts() {
    use crate::compute::{execute_prepared, prepare_execution};
    use crate::pipeline::TransposeMethod;

    let gpu = match super::GpuPool::ensure_init() {
        Ok(gpu) => gpu,
        Err(error) if error.to_string().contains("GPU adapter not available") => return,
        Err(error) => panic!("tiled RGB GPU initialization failed: {error}"),
    };
    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    for (width, height) in [(3u32, 4u32), (17, 20), (768, 772)] {
        let bytes: Vec<u8> = (0usize..width as usize * height as usize * 3)
            .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
            .collect();
        let source =
            DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, bytes.clone()).unwrap());
        for method in [
            TransposeMethod::Rotate90,
            TransposeMethod::Rotate270,
            TransposeMethod::Transpose,
            TransposeMethod::Transverse,
        ] {
            assert_eq!(
                super::packed_rgb_transpose_tiled_dispatch(
                    &method,
                    height,
                    width,
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                ),
                Some((height.div_ceil(16), width.div_ceil(16))),
            );
            let mut expected = vec![0; bytes.len()];
            // Independent forward scatter checks every native sample,
            // without sharing the shader's inverse mapping or tile order.
            for y in 0..height {
                for x in 0..width {
                    let (target_x, target_y) = match method {
                        TransposeMethod::Rotate90 => (y, width - 1 - x),
                        TransposeMethod::Rotate270 => (height - 1 - y, x),
                        TransposeMethod::Transpose => (y, x),
                        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
                        _ => unreachable!("only axis swaps are selected"),
                    };
                    let input_start = ((y * width + x) * 3) as usize;
                    let output_start = ((target_y * height + target_x) * 3) as usize;
                    expected[output_start..output_start + 3]
                        .copy_from_slice(&bytes[input_start..input_start + 3]);
                }
            }
            let ops = [PipelineOp::Transpose { method }];
            for mode in [None, Some("RGB"), Some("HSV")] {
                let prepared = prepare_execution(&ops, Some(Backend::Gpu)).unwrap();
                let actual = execute_prepared(&prepared, &ops, &source, mode).unwrap();
                assert_eq!(actual.dimensions(), (height, width));
                assert_eq!(actual.color(), crate::raster::ColorType::Rgb8);
                assert_eq!(actual.as_bytes(), expected);
                let receipt = Backend::take_pipeline_telemetry().expect("tiled RGB receipt");
                assert_eq!(receipt.0, Some(Backend::Gpu));
                assert_eq!(receipt.1, Backend::Gpu);
                assert_eq!(receipt.2, 1);
                assert_eq!(receipt.6, Some(1));
                assert_eq!(receipt.7, None);
                let resources = receipt.8.expect("tiled RGB transfer counters");
                let native_bytes = u64::from(width) * u64::from(height) * 3;
                assert_eq!(resources.upload_bytes, native_bytes);
                assert_eq!(resources.readback_bytes, native_bytes);
                assert_eq!(resources.parameter_bytes, 32);
                assert_eq!(resources.host_allocation_count, 1);
                assert_eq!(resources.host_allocated_bytes, native_bytes);
                assert_eq!(resources.mode_conversion_count, 0);
                assert_eq!(
                    resources.full_frame_copy_count,
                    if gpu.direct_primary_readback { 0 } else { 2 },
                );
            }
        }
    }
}
