use super::*;

#[test]
fn gpu_primary_readback_admission_excludes_unshared_or_unavailable_memory() {
    use wgpu::{Backend as Api, DeviceType as Kind, Features};
    let feature = Features::MAPPABLE_PRIMARY_BUFFERS;
    for (backend, device_type, features) in [
        (Api::Metal, Kind::IntegratedGpu, Features::empty()),
        (Api::Metal, Kind::DiscreteGpu, feature),
        (Api::Metal, Kind::VirtualGpu, feature),
        (Api::Metal, Kind::Cpu, feature),
        (Api::Metal, Kind::Other, feature),
        (Api::Vulkan, Kind::IntegratedGpu, feature),
        (Api::Dx12, Kind::IntegratedGpu, feature),
        (Api::Gl, Kind::IntegratedGpu, feature),
        (Api::BrowserWebGpu, Kind::IntegratedGpu, feature),
    ] {
        assert!(!super::device::direct_primary_readback_supported(
            backend,
            device_type,
            features
        ));
    }
    assert_eq!(
        super::device::direct_primary_readback_supported(Api::Metal, Kind::IntegratedGpu, feature),
        cfg!(not(target_arch = "wasm32")),
    );
}

#[test]
fn gpu_primary_readback_matches_staged_shader_results_and_reuse() {
    let Some(gpu) = primary_readback_test_gpu() else {
        return;
    };
    // Exercise each ping-pong result and a pipeline spanning two chunks.
    // Reuse the same owned working set with different input bytes each run.
    for direct in [false, true] {
        let mut buffers = super::BufferPool::new(&gpu.device, 17 * 15, direct);
        for operation_count in [1, 2, super::MAX_GPU_OPS_PER_SUBMISSION + 1] {
            assert_primary_readback_case(gpu, direct, operation_count, &mut buffers);
        }
    }
}

fn assert_primary_readback_case(
    gpu: &super::GpuInner,
    direct: bool,
    operation_count: usize,
    buffers: &mut super::BufferPool,
) {
    let (source, ops, expected) = primary_readback_case_input(operation_count);
    let auxiliary: Vec<_> = ops
        .iter()
        .map(|_| super::AuxiliaryImages {
            second: None,
            third: None,
        })
        .collect();
    buffers.upload_standard_image(&gpu.queue, &source).unwrap();
    let (final_is_a, width, height, readback, dispatches) =
        run_primary_readback_batch(gpu, &ops, &auxiliary, buffers);
    assert_primary_readback_result(
        gpu,
        direct,
        operation_count,
        buffers,
        &expected,
        final_is_a,
        width,
        height,
        readback,
        dispatches,
    );
}

fn primary_readback_case_input(
    operation_count: usize,
) -> (DynamicImage, Vec<PipelineOp>, DynamicImage) {
    use crate::compute::registry;
    use crate::pipeline::TransposeMethod;

    let bytes: Vec<u8> = (0usize..17 * 15 * 4)
        .map(|index| {
            index
                .wrapping_mul(113)
                .wrapping_add(index.checked_div(7).expect("nonzero fixture divisor"))
                .wrapping_add(operation_count) as u8
        })
        .collect();
    let source = DynamicImage::ImageRgba8(RgbaImage::from_raw(17, 15, bytes).unwrap());
    let mut ops = vec![PipelineOp::Transpose {
        method: TransposeMethod::Transpose,
    }];
    ops.extend((1..operation_count).map(|_| PipelineOp::Duplicate));
    let expected = ops.iter().fold(source.clone(), |image, op| {
        registry::execute_cpu(op, &image, Some("RGBA")).unwrap()
    });
    (source, ops, expected)
}

fn run_primary_readback_batch(
    gpu: &super::GpuInner,
    ops: &[PipelineOp],
    auxiliary: &[super::AuxiliaryImages],
    buffers: &mut super::BufferPool,
) -> (bool, u32, u32, super::ReadbackTarget, u64) {
    let (final_is_a, width, height, readback, _, dispatches, _) = gpu
        .execute_batch_impl(
            ops,
            auxiliary,
            17,
            15,
            3,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            None,
            None,
            Some("RGBA"),
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            buffers,
        )
        .unwrap();
    (final_is_a, width, height, readback, dispatches)
}

fn assert_primary_readback_result(
    gpu: &super::GpuInner,
    direct: bool,
    operation_count: usize,
    buffers: &mut super::BufferPool,
    expected: &DynamicImage,
    final_is_a: bool,
    width: u32,
    height: u32,
    readback: super::ReadbackTarget,
    dispatches: u64,
) {
    assert_eq!(dispatches, operation_count as u64);
    assert_eq!(final_is_a, operation_count.is_multiple_of(2));
    assert_eq!(matches!(readback, super::ReadbackTarget::Primary), direct);
    assert_eq!(
        readback.full_frame_copy_count(true),
        if direct { 1 } else { 2 }
    );
    assert_eq!(
        readback.full_frame_copy_count(false),
        if direct { 0 } else { 1 }
    );
    let actual = gpu
        .readback_to_image(width, height, readback.buffer(buffers, final_is_a), false)
        .unwrap();
    assert_eq!(actual.color(), expected.color());
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert_eq!(
        actual.as_bytes(),
        expected.as_bytes(),
        "direct={direct} ops={operation_count}"
    );
    // No returned image may retain a view into the next GPU write.
    gpu.queue.write_buffer(&buffers.buf_a, 0, &[0, 0, 0, 0]);
    gpu.queue.write_buffer(&buffers.buf_b, 0, &[0, 0, 0, 0]);
    gpu.queue.submit(std::iter::empty());
    gpu.poll_device("GPU readback reuse test").unwrap();
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    // These test-owned working sets never enter the global pool,
    // so staged resources cannot alter its immutable device policy.
}

#[test]
fn gpu_primary_readback_streamed_identity_flushes_pending_upload() {
    let Some(gpu) = primary_readback_test_gpu() else {
        return;
    };
    let (kernel, support) = super::filter_from_resample(ResampleFilter::Box);
    let horizontal = super::precompute_coeffs_f64(3, 3, kernel, support);
    let vertical = super::precompute_coeffs_f64(2, 2, kernel, support);
    let bytes: Vec<u8> = [
        0u32, 0x80000000, 0x7fc00123, 0x3fa00000, 0xff800000, 0x12345678,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();
    for direct in [false, true] {
        let buffers = super::BufferPool::new(&gpu.device, 6, direct);
        // Deliberately leave the upload pending: this zero-dispatch path
        // must submit it before map_async can expose the primary resource.
        gpu.queue.write_buffer(&buffers.buf_a, 0, &bytes);
        let (final_is_a, width, height, readback, _, dispatches) = gpu
            .execute_f_resize_tiled((3, 2), (3, 2), &horizontal, &vertical, &buffers)
            .unwrap();
        assert_eq!((final_is_a, width, height, dispatches), (true, 3, 2, 0));
        assert_eq!(matches!(readback, super::ReadbackTarget::Primary), direct);
        assert_eq!(
            gpu.readback_bytes(24, readback.buffer(&buffers, final_is_a))
                .unwrap(),
            bytes,
        );
    }
}

#[test]
fn gpu_primary_readback_concurrent_public_pipelines_keep_outputs_owned() {
    let Some(_gpu) = primary_readback_test_gpu() else {
        return;
    };
    // Reference bytes are computed before either GPU worker exists. Equal
    // pixel capacities with different shapes, modes, and values stress the
    // shared pool without allowing one worker's image to resemble another.
    let jobs: Vec<_> = (0usize..2).map(native_public_readback_cases).collect();
    // Enable telemetry in every worker: the parent's thread-local setting
    // is deliberately not inherited by newly spawned threads.
    let _telemetry = RestorePublicReadbackTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let start = std::sync::Barrier::new(2);
    let completed = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .into_iter()
            .enumerate()
            .map(|(worker, cases)| {
                let start = &start;
                scope.spawn(move || execute_public_readback_worker(worker, cases, start))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("GPU worker failed"))
            .collect::<Vec<_>>()
    });
    // Recheck only after both workers have finished all six uploads and
    // mappings, so every retained output survives the complete reuse run.
    for (image, bytes, expected) in completed.into_iter().flatten() {
        assert_eq!(bytes, expected);
        assert_eq!(image.tobytes().unwrap(), expected);
    }
}

type PublicReadbackCase = (&'static str, u32, u32, Vec<u8>, Vec<u8>);

struct RestorePublicReadbackTelemetry(bool);

impl Drop for RestorePublicReadbackTelemetry {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.0);
    }
}

fn native_public_readback_cases(worker: usize) -> Vec<PublicReadbackCase> {
    (0usize..6)
        .map(|iteration| {
            let (mode, channels) = if worker == 0 { ("RGB", 3) } else { ("RGBA", 4) };
            let (width, height) = if worker.wrapping_add(iteration).is_multiple_of(2) {
                (129usize, 131usize)
            } else {
                (131usize, 129usize)
            };
            let source: Vec<u8> = (0..width.saturating_mul(height).saturating_mul(channels))
                .map(|index| {
                    index
                        .wrapping_mul(113)
                        .wrapping_add(
                            index
                                .checked_div(channels)
                                .expect("readback channels are nonzero")
                                .wrapping_mul(7),
                        )
                        .wrapping_add(
                            index
                                .checked_div(width.saturating_mul(channels))
                                .expect("readback row width is nonzero")
                                .wrapping_mul(29),
                        )
                        .wrapping_add(worker.wrapping_mul(61))
                        .wrapping_add(iteration.wrapping_mul(43)) as u8
                })
                .collect();
            let mut expected = vec![0u8; source.len()];
            for output_y in 0..width {
                for output_x in 0..height {
                    let input_offset = output_x
                        .saturating_mul(width)
                        .saturating_add(output_y)
                        .saturating_mul(channels);
                    let output_offset = output_y
                        .saturating_mul(height)
                        .saturating_add(output_x)
                        .saturating_mul(channels);
                    expected[output_offset..output_offset.saturating_add(channels)]
                        .copy_from_slice(
                            &source[input_offset..input_offset.saturating_add(channels)],
                        );
                }
            }
            (mode, width as u32, height as u32, source, expected)
        })
        .collect()
}

fn execute_public_readback_worker(
    worker: usize,
    cases: Vec<PublicReadbackCase>,
    start: &std::sync::Barrier,
) -> Vec<(Image, Vec<u8>, Vec<u8>)> {
    let _telemetry = RestorePublicReadbackTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    // Only synchronize once: a later assertion failure cannot strand the peer.
    start.wait();
    let mut retained = Vec::new();
    for (iteration, (mode, width, height, source, expected)) in cases.into_iter().enumerate() {
        let pipeline = Image::frombytes(mode, (width, height), &source)
            .expect("concurrent native source")
            .transpose("TRANSPOSE")
            .expect("concurrent transpose construction")
            .use_backend(Backend::Gpu);
        let actual = pipeline.tobytes().expect("concurrent GPU materialization");
        let receipt = Backend::take_pipeline_telemetry()
            .expect("each worker must publish its own GPU receipt");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(
            receipt.1,
            Backend::Gpu,
            "worker={worker} iteration={iteration}"
        );
        assert_eq!(receipt.2, 1);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None, "native test must never accept fallback");
        let resources = receipt.8.expect("concurrent GPU resource receipt");
        assert_eq!(
            resources.full_frame_copy_count,
            if cfg!(target_endian = "little") { 0 } else { 1 },
        );
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        let transfer_bytes = if cfg!(target_endian = "little") && mode == "RGB" {
            pixels.saturating_mul(3).saturating_add(3) & !3
        } else {
            pixels.saturating_mul(4)
        };
        assert_eq!(resources.readback_bytes, transfer_bytes);
        assert_eq!(resources.upload_bytes, transfer_bytes);
        assert_eq!(resources.mode_conversion_count, 0);
        assert_eq!(actual, expected, "worker={worker} iteration={iteration}");
        assert_eq!(pipeline.size().unwrap(), (height, width));
        retained.push((pipeline, actual, expected));
        // Keep earlier public images and returned bytes alive while both workers reuse pooled storage.
        for (image, bytes, reference) in &retained {
            assert_eq!(bytes, reference);
            assert_eq!(image.tobytes().unwrap(), *reference);
        }
    }
    retained
}

#[test]
fn gpu_primary_readback_decode_error_unmaps_for_gpu_reuse() {
    let Some(gpu) = primary_readback_test_gpu() else {
        return;
    };
    let buffers = super::BufferPool::new(&gpu.device, 2, true);
    for buffer in [&buffers.buf_a, &buffers.buf_b] {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        gpu.queue.write_buffer(buffer, 0, &bytes);
        gpu.queue.submit(std::iter::empty());
        let failed = gpu.readback_with(8, buffer, |mapped| {
            super::rgb_from_packed_readback(1, 1, mapped)
        });
        assert!(failed.is_err(), "malformed decoded shape must fail");
        let next = [8, 7, 6, 5, 4, 3, 2, 1];
        // This submission would fail validation if decoding left the
        // primary resource mapped. Readback must observe the new upload.
        gpu.queue.write_buffer(buffer, 0, &next);
        gpu.queue.submit(std::iter::empty());
        assert_eq!(gpu.readback_bytes(8, buffer).unwrap(), next);
    }
}

#[test]
fn gpu_rgb_readback_matches_native_mode_preservation() {
    for (w, h) in [(1, 1), (3, 5), (17, 15), (257, 3), (513, 515)] {
        let rgba: Vec<u8> = (0usize..w as usize * h as usize * 4)
            .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(w, h, rgba.clone()).expect("RGBA readback source"),
        );
        let expected = image.to_rgb8();
        #[cfg(target_endian = "big")]
        let rgba: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|pixel| {
                u32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]).to_ne_bytes()
            })
            .collect();
        let actual = super::rgb_from_packed_readback(w, h, &rgba).expect("RGB readback extraction");
        assert_eq!(actual.color(), crate::raster::ColorType::Rgb8);
        assert_eq!(actual.dimensions(), (w, h));
        assert_eq!(actual.as_bytes(), expected.as_raw(), "{w}x{h}");
    }
    for bytes in [&[1, 2, 3][..], &[1, 2, 3, 4, 5][..]] {
        assert!(super::rgb_from_packed_readback(1, 1, bytes).is_err());
    }
    assert!(super::rgb_from_packed_readback(0, 1, &[]).is_err());
}

#[test]
fn gpu_rgb_readback_native_respects_final_mode_and_transfer_counts() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels, op) in native_rgb_readback_cases() {
        let Some((actual, expected)) = execute_native_rgb_readback_case(mode, channels, &op) else {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        };
        assert_native_rgb_readback_receipt(mode, &op, &actual, &expected);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn native_rgb_readback_cases() -> Vec<(&'static str, usize, PipelineOp)> {
    vec![
        ("RGB", 3, PipelineOp::Duplicate),
        (
            "RGB",
            3,
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            },
        ),
        (
            "RGBA",
            4,
            PipelineOp::Convert {
                mode: ColorMode::RGB,
                matrix: None,
                dither: None,
            },
        ),
        (
            "L",
            1,
            PipelineOp::Convert {
                mode: ColorMode::RGB,
                matrix: None,
                dither: None,
            },
        ),
        (
            "RGB",
            3,
            PipelineOp::Convert {
                mode: ColorMode::L,
                matrix: None,
                dither: None,
            },
        ),
        (
            "RGB",
            3,
            PipelineOp::PutAlpha {
                alpha: 37,
                mode: PixelMode::RGB,
            },
        ),
    ]
}

fn execute_native_rgb_readback_case(
    mode: &str,
    channels: usize,
    op: &PipelineOp,
) -> Option<(DynamicImage, DynamicImage)> {
    use crate::compute::{execute_prepared, prepare_execution, registry};

    let bytes: Vec<u8> = (0usize..channels.saturating_mul(15))
        .map(|index| index.wrapping_mul(29).wrapping_add(index / 3) as u8)
        .collect();
    let source = Image::frombytes(mode, (5, 3), &bytes)
        .expect("native readback source")
        .materialize()
        .expect("readback source materialization");
    let expected =
        registry::execute_cpu(op, &source, Some(mode)).expect("CPU native readback reference");
    let ops = std::slice::from_ref(op);
    let prepared = prepare_execution(ops, Some(Backend::Gpu)).expect("GPU routing");
    match execute_prepared(&prepared, ops, &source, Some(mode)) {
        Ok(actual) => Some((actual, expected)),
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            None
        }
        Err(error) => panic!("native GPU {mode} readback failed: {error}"),
    }
}

fn assert_native_rgb_readback_receipt(
    mode: &str,
    op: &PipelineOp,
    actual: &DynamicImage,
    expected: &DynamicImage,
) {
    assert_eq!(actual.color(), expected.color(), "{mode} {op:?}");
    assert_eq!(actual.as_bytes(), expected.as_bytes(), "{mode} {op:?}");
    assert_native_rgb_readback_resources(mode, op, actual);
}

fn assert_native_rgb_readback_resources(mode: &str, op: &PipelineOp, actual: &DynamicImage) {
    let telemetry = Backend::take_pipeline_telemetry().expect("readback receipt");
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    let resources = telemetry.8.expect("native transfer counters");
    let native_rgb_duplicate =
        cfg!(target_endian = "little") && mode == "RGB" && matches!(op, PipelineOp::Duplicate);
    let native_three_channel_upload = native_rgb_duplicate
        || (cfg!(target_endian = "little")
            && matches!(
                op,
                PipelineOp::PutAlpha {
                    mode: PixelMode::RGB,
                    ..
                }
            ))
        || (cfg!(target_endian = "little")
            && mode == "RGB"
            && matches!(
                op,
                PipelineOp::Convert {
                    mode: ColorMode::RGBA,
                    matrix: None,
                    dither: None,
                }
            ));
    let expected_upload_bytes = if native_three_channel_upload {
        (5 * 3 * 3 + 3) & !3
    } else {
        5 * 3 * 4
    };
    assert_eq!(
        resources.upload_bytes, expected_upload_bytes,
        "{mode} {op:?}"
    );
    let expected_readback_bytes = if native_rgb_duplicate {
        (5 * 3 * 3 + 3) & !3
    } else {
        5 * 3 * 4
    };
    assert_eq!(
        resources.readback_bytes, expected_readback_bytes,
        "{mode} {op:?}"
    );
    if actual.color() == crate::raster::ColorType::Rgb8 {
        assert_eq!(resources.host_allocation_count, 1);
        assert_eq!(resources.host_allocated_bytes, 5 * 3 * 3);
    }
    if mode == "RGB"
        && matches!(
            op,
            PipelineOp::Convert {
                mode: ColorMode::RGBA,
                matrix: None,
                dither: None,
            }
        )
    {
        assert_eq!(resources.mode_conversion_count, 0);
    }
}

#[test]
fn gpu_rgb_readback_decode_error_unmaps_staging() {
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
        Err(error) => panic!("GPU readback initialization failed: {error}"),
    };
    let staging = super::StagingBuffer::new(&gpu.device, 8);
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
    gpu.queue.write_buffer(&staging.buffer, 0, &bytes);
    gpu.queue.submit(std::iter::empty());
    let failed = gpu.readback_with(8, &staging.buffer, |mapped| {
        super::rgb_from_packed_readback(1, 1, mapped)
    });
    assert!(failed.is_err(), "malformed decoded shape must fail");
    assert_eq!(
        gpu.readback_bytes(8, &staging.buffer)
            .expect("staging must be unmapped after decoder error"),
        bytes
    );
}
