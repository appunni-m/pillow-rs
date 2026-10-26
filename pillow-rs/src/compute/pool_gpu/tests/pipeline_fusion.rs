use super::*;

#[test]
fn gpu_transpose_fusion_preserves_shape_changing_prefixes() {
    use crate::compute::registry;
    use crate::pipeline::TransposeMethod;

    let source = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![7, 29, 131, 53]).unwrap());
    let ops = [
        PipelineOp::Resize {
            w: 3,
            h: 2,
            filter: ResampleFilter::Nearest,
        },
        PipelineOp::Transpose {
            method: TransposeMethod::FlipTopBottom,
        },
        PipelineOp::Transpose {
            method: TransposeMethod::Rotate90,
        },
    ];
    let sequential = |ops: &[PipelineOp]| {
        ops.iter()
            .try_fold(source.clone(), |image, op| {
                registry::execute_cpu(op, &image, None)
            })
            .unwrap()
    };
    let expected = sequential(&ops);
    let fused = super::fuse_gpu_transpose_ops(&ops);
    let actual = sequential(&fused);
    assert_eq!(fused.len(), 2);
    assert_eq!(expected.dimensions(), (2, 3));
    assert_eq!(actual.dimensions(), expected.dimensions());
    assert_eq!(actual.as_bytes(), expected.as_bytes());
}

#[test]
fn gpu_crop_border_fusion_matches_ordered_execution() {
    use crate::compute::registry;

    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            11,
            11,
            (0..11 * 11 * 4)
                .map(|index| (index * 37 + index / 11) as u8)
                .collect(),
        )
        .unwrap(),
    );
    let ops = [
        PipelineOp::CropBorder { border: 1 },
        PipelineOp::CropBorder { border: 2 },
        PipelineOp::Invert,
        PipelineOp::CropBorder { border: 0 },
        PipelineOp::CropBorder { border: 1 },
    ];
    let sequential = |ops: &[PipelineOp]| {
        ops.iter()
            .try_fold(source.clone(), |image, op| {
                registry::execute_cpu(op, &image, None)
            })
            .unwrap()
    };
    let expected = sequential(&ops);
    let fused_ops = super::fuse_gpu_crop_border_ops(&ops);
    let actual = sequential(&fused_ops);
    assert_eq!(fused_ops.len(), 3);
    assert_eq!(expected.dimensions(), actual.dimensions());
    assert_eq!(expected.as_bytes(), actual.as_bytes());
}

#[test]
fn gpu_transpose_fusion_covers_every_pair_and_identity_continuation() {
    use crate::compute::registry;
    use crate::pipeline::TransposeMethod;

    let methods = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    for (width, height) in [(0, 3), (3, 0), (1, 7), (9, 1), (5, 3)] {
        let bytes = (0..width as usize * height as usize * 4)
            .map(|index| index.wrapping_mul(113).wrapping_add(index / 7) as u8)
            .collect();
        let source = DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, bytes).unwrap());
        for first in &methods {
            for second in &methods {
                let pair = [
                    PipelineOp::Transpose {
                        method: first.clone(),
                    },
                    PipelineOp::Transpose {
                        method: second.clone(),
                    },
                ];
                for ops in [pair.to_vec(), [pair.as_slice(), &pair].concat()] {
                    let sequential = |ops: &[PipelineOp]| {
                        ops.iter()
                            .try_fold(source.clone(), |image, op| {
                                registry::execute_cpu(op, &image, None)
                            })
                            .unwrap()
                    };
                    let expected = sequential(&ops);
                    let fused = super::fuse_gpu_transpose_ops(&ops);
                    let actual = sequential(&fused);
                    assert!(fused.len() <= 1);
                    assert_eq!(
                        actual.dimensions(),
                        expected.dimensions(),
                        "{width}x{height} {ops:?}"
                    );
                    assert_eq!(
                        actual.as_bytes(),
                        expected.as_bytes(),
                        "{width}x{height} {ops:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn gpu_transpose_fusion_native_preserves_logical_modes_and_dispatch_counts() {
    use crate::compute::{execute_prepared, prepare_execution, registry};
    use crate::pipeline::TransposeMethod;

    let methods = [
        TransposeMethod::FlipLeftRight,
        TransposeMethod::FlipTopBottom,
        TransposeMethod::Rotate90,
        TransposeMethod::Rotate180,
        TransposeMethod::Rotate270,
        TransposeMethod::Transpose,
        TransposeMethod::Transverse,
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channels) in [
        ("L", 1),
        ("LA", 2),
        ("RGB", 3),
        ("RGBA", 4),
        ("P", 1),
        ("PA", 2),
        ("I", 4),
        ("F", 4),
        ("RGBX", 4),
    ] {
        let bytes: Vec<u8> = (0..5 * 3 * channels)
            .map(|index: usize| index.wrapping_mul(29).wrapping_add(index / 3) as u8)
            .collect();
        let source = Image::frombytes(mode, (5, 3), &bytes)
            .expect("transpose source")
            .materialize()
            .expect("native transpose source");
        for first in &methods {
            for second in &methods {
                let ops = [
                    PipelineOp::Transpose {
                        method: first.clone(),
                    },
                    PipelineOp::Transpose {
                        method: second.clone(),
                    },
                ];
                let middle = registry::execute_cpu(&ops[0], &source, Some(mode))
                    .expect("first CPU transpose");
                let expected = registry::execute_cpu(&ops[1], &middle, Some(mode))
                    .expect("second CPU transpose");
                // Image::push_op intentionally puts each PA operation in
                // a separate materialization node to preserve palette
                // layout boundaries. That public chain reports a final
                // one-op receipt and cannot exercise two-op GPU fusion.
                // Submit the explicit batch through the ordinary routed
                // execution boundary so every mode must preserve both
                // public operations in this receipt.
                let prepared = prepare_execution(&ops, Some(Backend::Gpu))
                    .expect("GPU transpose batch routing");
                let actual = match execute_prepared(&prepared, &ops, &source, Some(mode)) {
                    Ok(actual) => actual,
                    Err(error)
                        if error.to_string().contains("GPU adapter not available")
                            || error
                                .to_string()
                                .contains("GPU device initialization failed") =>
                    {
                        Backend::set_pipeline_telemetry_enabled(previous);
                        return;
                    }
                    Err(error) => panic!("native GPU {mode} transpose chain failed: {error}"),
                };
                assert_eq!(
                    actual.dimensions(),
                    expected.dimensions(),
                    "{mode} {first:?} {second:?}"
                );
                assert_eq!(
                    actual.as_bytes(),
                    expected.as_bytes(),
                    "{mode} {first:?} {second:?}"
                );
                let telemetry = Backend::take_pipeline_telemetry().expect("GPU transpose receipt");
                assert_eq!(telemetry.0, Some(Backend::Gpu));
                assert_eq!(telemetry.1, Backend::Gpu);
                assert_eq!(telemetry.2, 2, "{mode} {first:?} {second:?}");
                assert_eq!(
                    telemetry.6,
                    Some(super::fuse_gpu_transpose_ops(&ops).len() as u64)
                );
                assert_eq!(telemetry.7, None);
            }
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn gpu_transpose_public_pa_chain_preserves_materialization_boundary() {
    use crate::pipeline::TransposeMethod;

    let bytes: Vec<u8> = (0usize..5 * 3 * 2)
        .map(|index| index.wrapping_mul(29).wrapping_add(index / 3) as u8)
        .collect();
    let source = Image::frombytes("PA", (5, 3), &bytes).expect("PA transpose source");
    let first = Image::push_op(
        &source,
        PipelineOp::Transpose {
            method: TransposeMethod::FlipLeftRight,
        },
    );
    let chain = Image::push_op(
        &first,
        PipelineOp::Transpose {
            method: TransposeMethod::Rotate90,
        },
    );
    let expected = chain
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU PA transpose chain");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match chain.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
        Err(error) => panic!("native GPU public PA transpose chain failed: {error}"),
    };
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry().expect("public PA transpose receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    // PA is two separately routed one-op nodes; the receipt describes
    // the terminal Rotate90 rather than an impossible combined batch.
    assert_eq!(telemetry.2, 1);
    assert_eq!(telemetry.6, Some(1));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn ordered_f64_state_covers_subnormal_overflow_and_nan_boundaries() {
    assert_ordered_f64_subnormal_states();
    assert_ordered_f64_cancellation_state();
    assert_ordered_f64_overflow_and_nan_states();
}

fn assert_ordered_f64_subnormal_states() {
    let smallest = gpu_f64_ordered_round(
        F64SignedMagnitude {
            magnitude: 1,
            negative: true,
        },
        -1074,
    )
    .expect("smallest binary64 subnormal");
    assert_eq!(smallest.kind, F64OrderedKind::Finite);
    assert_eq!(smallest.magnitude, 1);
    assert_eq!(smallest.exponent, -1074);
    assert!(smallest.negative);
    assert_eq!(gpu_f64_ordered_state_to_f32(smallest), Some(1u32 << 31));

    let underflow = gpu_f64_ordered_round(
        F64SignedMagnitude {
            magnitude: 1,
            negative: true,
        },
        -1075,
    )
    .expect("binary64 underflow");
    assert_eq!(underflow.kind, F64OrderedKind::Finite);
    assert_eq!(underflow.magnitude, 0);
    assert!(underflow.negative);
    assert_eq!(gpu_f64_ordered_state_to_f32(underflow), Some(1u32 << 31));
}

fn assert_ordered_f64_cancellation_state() {
    let positive = gpu_f64_ordered_add_product(
        F64OrderedState {
            magnitude: 0,
            exponent: 0,
            negative: false,
            kind: F64OrderedKind::Finite,
        },
        1,
        0,
        false,
        false,
    )
    .expect("positive product");
    let cancelled = gpu_f64_ordered_add_product(positive, 1, 0, true, false)
        .expect("opposite product cancellation");
    assert_eq!(cancelled.kind, F64OrderedKind::Finite);
    assert_eq!(cancelled.magnitude, 0);
    assert!(!cancelled.negative);
    assert_eq!(gpu_f64_ordered_state_to_f32(cancelled), Some(0));
}

fn assert_ordered_f64_overflow_and_nan_states() {
    let positive_infinity = gpu_f64_ordered_add_product(
        F64OrderedState {
            magnitude: 0,
            exponent: 0,
            negative: false,
            kind: F64OrderedKind::Finite,
        },
        1,
        1024,
        false,
        false,
    )
    .expect("binary64 overflow");
    assert_eq!(positive_infinity.kind, F64OrderedKind::Infinity);
    assert_eq!(
        gpu_f64_ordered_state_to_f32(positive_infinity),
        Some(0x7f80_0000)
    );

    let nan = gpu_f64_ordered_add_product(positive_infinity, 1, 1024, true, true)
        .expect("opposite binary64 infinities");
    assert_eq!(nan.kind, F64OrderedKind::Nan);
    assert_eq!(gpu_f64_ordered_state_to_f32(nan), Some(0x7fc0_0000));
}

#[test]
fn grayscale_f_prefix_keeps_gpu_suffix_receipt_terminal() {
    let source_bytes = [
        1.5f32.to_le_bytes(),
        (-2.25f32).to_le_bytes(),
        300.0f32.to_le_bytes(),
        0.0f32.to_le_bytes(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let source = Image::frombytes("F", (2, 2), &source_bytes).expect("F source");
    let pipeline = crate::ops::imageops::invert(
        &crate::ops::imageops::grayscale(&source).expect("grayscale pipeline"),
    )
    .expect("invert pipeline");
    let expected = pipeline
        .clone()
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU grayscale/invert");
    assert_eq!(expected, vec![0xfe, 0xff, 0x00, 0xff]);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match pipeline.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
        Err(error) => panic!("native GPU grayscale/invert failed: {error}"),
    };
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("segmented grayscale/invert must publish a receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.2, 2);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(
        telemetry.7.as_deref(),
        Some("exact host semantic control: GPU logical-mode operation contract is not proven")
    );
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
#[cfg(target_endian = "little")]
fn ycbcr_grayscale_matches_cpu_for_complete_groups_and_pixel_tails() {
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
        Err(error) => panic!("GPU YCbCr grayscale initialization failed: {error}"),
    }

    let operation = PipelineOp::Grayscale;
    for (width, height) in [(1u32, 1u32), (2, 1), (3, 1), (4, 1), (5, 1), (3, 3)] {
        assert_ycbcr_grayscale_case(&operation, width, height);
    }
}

#[cfg(target_endian = "little")]
fn assert_ycbcr_grayscale_case(operation: &PipelineOp, width: u32, height: u32) {
    use crate::compute::BackendImpl;

    let pixel_count = width as usize * height as usize;
    let bytes = (0..pixel_count)
        .flat_map(|pixel| {
            [
                pixel.wrapping_mul(47).wrapping_add(13) as u8,
                pixel.wrapping_mul(29).wrapping_add(71) as u8,
                pixel.wrapping_mul(83).wrapping_add(19) as u8,
            ]
        })
        .collect();
    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, bytes).expect("YCbCr grayscale source"),
    );
    let expected = crate::compute::registry::execute_cpu(operation, &source, Some("YCbCr"))
        .unwrap_or_else(|error| panic!("CPU YCbCr grayscale {width}x{height}: {error}"));
    let _ = Backend::take_pipeline_allocation_telemetry();
    let actual = super::GpuPool
        .execute_batch_strict(std::slice::from_ref(operation), &source, Some("YCbCr"))
        .unwrap_or_else(|error| panic!("GPU YCbCr grayscale {width}x{height}: {error}"));
    assert!(matches!(&actual, DynamicImage::ImageLuma8(_)));
    assert_eq!(
        actual.as_bytes(),
        expected.as_bytes(),
        "{width}x{height} pixels"
    );

    assert_ycbcr_grayscale_resource_receipts(width, height, pixel_count);
}

#[cfg(target_endian = "little")]
fn assert_ycbcr_grayscale_resource_receipts(width: u32, height: u32, pixel_count: usize) {
    let resources = crate::compute::take_pipeline_resource_telemetry()
        .unwrap_or_else(|| panic!("{width}x{height} resource receipt"));
    let allocations = Backend::take_pipeline_allocation_telemetry();
    let upload_bytes = (pixel_count * 3).div_ceil(4) * 4;
    let readback_bytes = pixel_count.div_ceil(4) * 4;
    assert_eq!(
        resources.upload_bytes, upload_bytes as u64,
        "{width}x{height} upload"
    );
    assert_eq!(
        resources.readback_bytes, readback_bytes as u64,
        "{width}x{height} readback"
    );
    assert_eq!(
        resources.mode_conversion_count, 0,
        "{width}x{height} conversion"
    );
    assert_eq!(
        allocations.allocation_count, 1,
        "{width}x{height} allocations"
    );
    assert_eq!(
        allocations.allocated_bytes, pixel_count as u64,
        "{width}x{height} allocated bytes"
    );
    assert_eq!(
        crate::compute::take_pipeline_dispatch_count(),
        Some(1),
        "{width}x{height} dispatch"
    );
    assert_eq!(
        crate::compute::take_pipeline_backend_override(),
        None,
        "{width}x{height} must execute on GPU without fallback"
    );
}
