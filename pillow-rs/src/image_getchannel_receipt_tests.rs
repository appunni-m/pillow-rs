//! Focused GetChannel correctness and allocation receipt regressions.
#![cfg(not(feature = "parallel"))]

use super::Image;
use crate::compute::{self, Backend, PipelineOperationTelemetry, PipelineResourceTelemetry};
use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp};
use std::sync::{Mutex, MutexGuard};

type PipelineReceipt = (
    Option<Backend>,
    Backend,
    usize,
    u64,
    u64,
    u64,
    Option<u64>,
    Option<String>,
    Option<PipelineResourceTelemetry>,
    u64,
    u64,
);

const RGB_BYTES: [u8; 18] = [
    11, 22, 33, 44, 55, 66, 77, 88, 99, 111, 122, 133, 144, 155, 166, 177, 188, 199,
];
const LA_BYTES: [u8; 12] = [10, 210, 20, 220, 30, 230, 40, 240, 50, 250, 60, 200];

static BACKEND_TEST_LOCK: Mutex<()> = Mutex::new(());

struct CpuOnlyBackendsGuard {
    previous: Vec<Backend>,
    _serial: MutexGuard<'static, ()>,
}

impl CpuOnlyBackendsGuard {
    fn new() -> Result<Self, PilError> {
        let serial = BACKEND_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = compute::active_backends()?;
        let guard = Self {
            previous,
            _serial: serial,
        };

        for backend in [Backend::Cpu, Backend::Gpu, Backend::Simd] {
            compute::disable_backend(backend)?;
        }
        compute::enable_backend(Backend::Cpu)?;
        Ok(guard)
    }
}

impl Drop for CpuOnlyBackendsGuard {
    fn drop(&mut self) {
        for backend in [Backend::Cpu, Backend::Gpu, Backend::Simd] {
            let _ = compute::disable_backend(backend);
        }
        for backend in self.previous.iter().copied() {
            let _ = compute::enable_backend(backend);
        }
    }
}

struct PipelineTelemetryGuard {
    previous: bool,
}

impl PipelineTelemetryGuard {
    fn enable() -> Self {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let _ = Backend::take_pipeline_telemetry();
        let _ = Backend::take_pipeline_operation_telemetry();
        Self { previous }
    }
}

impl Drop for PipelineTelemetryGuard {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.previous);
    }
}

fn capture_telemetry<T>(
    call: impl FnOnce() -> Result<T, PilError>,
) -> (
    Result<T, PilError>,
    Option<PipelineReceipt>,
    Vec<PipelineOperationTelemetry>,
) {
    let telemetry = PipelineTelemetryGuard::enable();
    let result = call();
    let receipt = Backend::take_pipeline_telemetry();
    let operations = Backend::take_pipeline_operation_telemetry();
    drop(telemetry);
    (result, receipt, operations)
}

#[test]
fn cpu_getchannel_preserves_alpha_through_native_la_raw_prefix() -> Result<(), PilError> {
    let _backends = CpuOnlyBackendsGuard::new()?;
    let root = Image::frombytes("LA", (3, 2), &LA_BYTES)?;
    let mut pipeline = root.clone();

    for _ in 0..2 {
        pipeline = Image::push_op(
            &pipeline,
            PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (10, 0, 0, 210),
                palette_index: false,
            },
        );
    }
    pipeline = Image::push_op(
        &pipeline,
        PipelineOp::Convert {
            mode: ColorMode::LA,
            matrix: None,
            dither: None,
        },
    )
    .use_backend(Backend::Cpu);

    let alpha = pipeline.getchannel(1)?;
    assert_eq!(alpha.tobytes()?, vec![210, 220, 230, 240, 250, 200]);
    Ok(())
}

#[test]
fn loaded_tiny_rgb_cpu_getchannel_reports_one_output_buffer_allocation() -> Result<(), PilError> {
    let _backends = CpuOnlyBackendsGuard::new()?;
    let image = Image::frombytes("RGB", (3, 2), &RGB_BYTES)?;

    let (result, receipt, operations) = capture_telemetry(|| image.getchannel(1));
    let output = result?;
    assert_eq!(output.mode()?, "L");
    assert_eq!(output.size()?, (3, 2));
    assert_eq!(output.tobytes()?, vec![22, 55, 88, 122, 155, 188]);

    let receipt = receipt.expect("eager CPU GetChannel receipt");
    assert_eq!(receipt.0, Some(Backend::Cpu));
    assert_eq!(receipt.1, Backend::Cpu);
    assert_eq!(receipt.2, 1);
    assert_eq!(receipt.7, None);

    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation, "ExtractBand");
    assert_eq!(operations[0].path, "cpu");
    assert_eq!(operations[0].mode_conversion_count, 1);

    let resources = receipt.8.expect("CPU host allocation receipt");
    assert_eq!(resources.mode_conversion_count, 1);
    assert_eq!(resources.host_allocation_count, 1);
    assert_eq!(resources.host_allocated_bytes, 6);
    Ok(())
}

#[test]
fn loaded_tiny_rgb_cpu_grayscale_reports_actual_mode_conversion() -> Result<(), PilError> {
    let _backends = CpuOnlyBackendsGuard::new()?;
    let image = Image::frombytes("RGB", (2, 1), &[0, 0, 0, 255, 255, 255])?;

    let (result, receipt, operations) =
        capture_telemetry(|| crate::ops::imageops::grayscale(&image));
    let output = result?;
    assert!(matches!(output, Image::Loaded(_)));
    assert_eq!(output.mode()?, "L");
    assert_eq!(output.tobytes()?, vec![0, 255]);

    let receipt = receipt.expect("eager CPU Grayscale receipt");
    assert_eq!(receipt.0, Some(Backend::Cpu));
    assert_eq!(receipt.1, Backend::Cpu);
    assert_eq!(receipt.2, 1);
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("CPU Grayscale resource receipt");
    assert_eq!(resources.mode_conversion_count, 1);
    assert_eq!(resources.host_buffer_count, 2);
    assert_eq!(resources.host_buffer_bytes, 8);
    assert_eq!(resources.host_allocation_count, 1);
    assert_eq!(resources.host_allocated_bytes, 2);
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation, "Grayscale");
    assert_eq!(operations[0].path, "cpu");
    assert_eq!(operations[0].mode_conversion_count, 1);
    assert_eq!(operations[0].handoff_count, 0);
    Ok(())
}

#[test]
fn direct_cpu_eager_same_mode_records_no_conversion() -> Result<(), PilError> {
    let image = Image::frombytes("L", (2, 1), &[17, 231])?;
    let input = image.materialize()?;
    let (result, receipt, operations) = capture_telemetry(|| {
        compute::execute_eager_cpu_image_operation("Grayscale", &input, false, || {
            compute::registry::execute_cpu(&PipelineOp::Grayscale, &input, Some("L"))
        })
    });
    assert_eq!(result?.as_bytes(), &[17, 231]);
    let receipt = receipt.expect("same-mode CPU resource receipt");
    assert_eq!(receipt.1, Backend::Cpu);
    assert_eq!(receipt.2, 1);
    assert_eq!(receipt.7, None);
    assert_eq!(
        receipt
            .8
            .expect("same-mode resources")
            .mode_conversion_count,
        0
    );
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation, "Grayscale");
    assert_eq!(operations[0].path, "cpu");
    assert_eq!(operations[0].mode_conversion_count, 0);
    Ok(())
}

#[test]
fn failed_cpu_eager_operation_clears_mode_conversion_receipt() -> Result<(), PilError> {
    let image = Image::frombytes("RGB", (2, 1), &[0, 0, 0, 255, 255, 255])?;
    let input = image.materialize()?;
    let (result, receipt, operations) = capture_telemetry(|| {
        compute::execute_eager_cpu_image_operation("Grayscale", &input, false, || {
            compute::registry::execute_cpu(&PipelineOp::Grayscale, &input, Some("RGB"))
        })?;
        compute::execute_eager_cpu_image_operation("Grayscale", &input, false, || {
            Err(PilError::ValueError("receipt test failure".into()))
        })
    });
    assert!(
        matches!(result, Err(PilError::ValueError(message)) if message == "receipt test failure")
    );
    assert!(receipt.is_none());
    assert!(operations.is_empty());
    Ok(())
}
