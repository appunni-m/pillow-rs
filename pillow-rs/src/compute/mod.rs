//! Compute backend routing for deferred image pipelines.
//!
//! This module decides where a materialized [`crate::pipeline::PipelineOp`]
//! batch executes. The public surface is intentionally small: callers can
//! inspect compiled backends, enable or disable automatic routing choices, lock
//! an image pipeline to one backend, and execute a selected backend.
//!
//! # Routing Contract
//!
//! Automatic materialization selects GPU when its operation contracts allow
//! it, otherwise CPU. CPU receives the host architecture in its execution
//! context and each operation selects a matching specialization or its
//! same-mode Default implementation. `Backend::Simd` remains a compatibility
//! request that selects CPU with the detected host architecture; it is not a
//! separate executor. GPU remains an independent device path.
//!
//! # Adding Operations
//!
//! New operations must define the pipeline variant, CPU mode/architecture
//! strategies, registry key, and optional GPU shader together. The operation's
//! support declaration is shared by support reporting, admission, and
//! prepared execution.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(crate) mod helpers;
#[path = "backend_state.rs"]
mod backend_state;
mod eager;
mod execution;
mod execution_context;
pub use execution_context::{Architecture, ExecutionContext};
#[path = "phase_telemetry.rs"]
mod phase_telemetry;
#[path = "telemetry.rs"]
mod telemetry;
#[cfg(test)]
#[path = "transpose_tests.rs"]
mod transpose_tests;

#[doc(hidden)]
pub use telemetry::PipelineTelemetryOrigin;
pub use telemetry::{
    GpuShaderDispatchTelemetry, PipelineAllocationTelemetry, PipelineOperationTelemetry,
    PipelineResourceTelemetry,
};
use telemetry::{
    PipelineTelemetry, elapsed_ns, pipeline_telemetry_enabled, pipeline_timestamp,
    record_pipeline_telemetry, set_gpu_shader_coverage_enabled, set_pipeline_telemetry_enabled,
    take_gpu_shader_coverage, take_pipeline_backend_override, take_pipeline_dispatch_count,
    take_pipeline_resize_coeff_cache_stats, take_pipeline_resource_telemetry,
    take_pipeline_telemetry,
};
pub(crate) use telemetry::{
    RestorePipelineOperationOrigins, account_host_buffer_boundary,
    begin_eager_cpu_operation_telemetry, begin_pipeline_operation_telemetry,
    cancel_eager_cpu_operation_telemetry, capture_active_origin, enter_operation_origins,
    finish_eager_cpu_operation_telemetry,
    finish_eager_cpu_operation_telemetry_with_selection,
    finish_pipeline_operation_telemetry, host_resource_telemetry,
    record_eager_cpu_operation_receipt, record_eager_cpu_operation_receipt_with_reason,
    record_pipeline_allocation, record_pipeline_backend_fallback,
    clear_pipeline_operation_architecture, record_pipeline_operation_architecture,
    record_pipeline_operation_handoff, record_pipeline_operation_mode_conversion,
    record_pipeline_operation_path, record_pipeline_operation_scalar_tail,
    record_pipeline_operation_unsupported, record_pipeline_operation_vector_blocks,
    record_pipeline_resize_coeff_cache_hit, record_pipeline_resize_coeff_cache_miss,
    record_pipeline_resource_telemetry, reset_pipeline_allocation_telemetry,
    reset_pipeline_operation_telemetry, take_pipeline_allocation_telemetry,
    take_pipeline_operation_telemetry,
};
#[cfg(feature = "gpu")]
pub(crate) use telemetry::{
    record_gpu_shader_dispatch, record_pipeline_dispatch_count, restore_pipeline_backend_override,
};

pub use backend_state::{
    active_backends, available_backends, backend_enabled, disable_backend, enable_backend,
};
pub(crate) use backend_state::{
    cpu_architecture_alias_is_only_active_backend, cpu_is_only_active_backend,
    preferred_active_backend,
};
pub(crate) use eager::{
    execute_eager_cpu_getprojection, execute_eager_cpu_linear_gradient,
    execute_eager_cpu_mandelbrot, execute_eager_cpu_radial_gradient,
    execute_eager_cpu_arch_bytes, execute_eager_cpu_arch_image_operation,
    execute_eager_cpu_arch_image_operation_with_route, execute_eager_cpu_image_operation,
    execute_eager_cpu_shared_result,
};
#[cfg(feature = "gpu")]
pub(crate) use eager::{
    execute_eager_gpu_rgb_getprojection, gpu_cmyk_projection_terminal_supported,
    gpu_rgb_projection_supported, gpu_rgb_projection_terminal_supported,
};
pub(crate) use execution::{
    PreparedExecution, execute_prepared, execute_prepared_cpu_arch_operation,
    execute_prepared_cpu_arch_terminal, execute_prepared_cpu_terminal, execute_prepared_shared,
    execute_prepared_shared_with_palette,
    execute_prepared_with_palette, prepare_execution,
};
#[cfg(feature = "gpu")]
pub(crate) use execution::{
    execute_prepared_gpu_cmyk_projection_terminal, execute_prepared_gpu_rgb_projection_terminal,
    merge_pipeline_resource_telemetry,
};
#[cfg(target_arch = "x86_64")]
pub(crate) use phase_telemetry::record_transpose_avx2_executor_timing;
pub(crate) use phase_telemetry::{
    pipeline_phase_timestamp, record_transpose_output_allocation_timing,
};

/// Return `(multiplier, offset)` when a 256-entry byte LUT is exactly the
/// modulo-256 affine mapping `sample * multiplier + offset`.
///
/// Point operations still use their full lookup tables for arbitrary maps.
/// This recognition lets the execution backends replace common linear LUTs
/// with integer multiply/add while preserving every table entry exactly.
pub(crate) fn byte_lut_affine_coefficients(lut: &[u8]) -> Option<(u8, u8)> {
    if lut.len() != 256 {
        return None;
    }
    let offset = lut[0];
    let multiplier = lut[1].wrapping_sub(offset);
    for (sample_index, &expected) in lut.iter().enumerate() {
        let sample = u8::try_from(sample_index).ok()?;
        if sample.wrapping_mul(multiplier).wrapping_add(offset) != expected {
            return None;
        }
    }
    Some((multiplier, offset))
}

// ── Backend ─────────────────────────────────────────────────────────────────

/// Compute backend used to execute a materialized image pipeline.
///
/// GPU is a separate executor; CPU selects architecture-specific strategies
/// from the injected context. `Simd` is retained as a compatibility request
/// that routes through CPU with the detected host architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Backend {
    /// Scalar CPU implementation. This backend is always available.
    Cpu = 0,
    /// WebGPU/wgpu implementation when the `gpu` feature is enabled.
    Gpu = 1,
    /// Compatibility alias for CPU execution with the detected architecture.
    Simd = 2,
}

/// One completed pipeline timing sample and its backend/resource counters.
///
/// The fields follow the order documented by [`Backend::take_pipeline_telemetry`].
pub type PipelineTelemetrySample = (
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

impl Backend {
    /// Parses a backend name used by CLI, environment, or binding-layer options.
    ///
    /// Accepted values are `"cpu"`, `"gpu"`, and the legacy `"simd"` alias,
    /// case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            "simd" => Some(Self::Simd),
            _ => None,
        }
    }

    /// Enables or disables bounded pipeline execution telemetry on this thread.
    ///
    /// Telemetry is disabled by default. The return value is the previous
    /// setting. When enabled, one completed sample is retained on this thread;
    /// recording a new sample replaces this thread's previous sample. Worker
    /// threads collect telemetry only when they enable it themselves.
    pub fn set_pipeline_telemetry_enabled(enabled: bool) -> bool {
        set_pipeline_telemetry_enabled(enabled)
    }

    /// Returns whether pipeline execution telemetry is enabled.
    pub fn pipeline_telemetry_enabled() -> bool {
        pipeline_telemetry_enabled()
    }

    /// Set the public workflow call used to label subsequent telemetry.
    ///
    /// Bindings and migration runners use this diagnostic context to keep
    /// lazy operation receipts attached to the call that created each op.
    #[doc(hidden)]
    pub fn set_pipeline_telemetry_origin(
        origin: Option<PipelineTelemetryOrigin>,
    ) -> Option<PipelineTelemetryOrigin> {
        telemetry::replace_active_origin(origin)
    }

    /// Clears the safe allocation counters used by the managed release
    /// benchmark. These counters cover image buffers allocated through the
    /// checked pixel-buffer boundary; they are not a process-global allocator
    /// hook and do not alter execution.
    pub fn reset_pipeline_allocation_telemetry() {
        reset_pipeline_allocation_telemetry()
    }

    /// Takes the safe allocation counters collected on this thread.
    pub fn take_pipeline_allocation_telemetry() -> PipelineAllocationTelemetry {
        take_pipeline_allocation_telemetry()
    }

    /// Takes the most recent completed pipeline telemetry sample for this
    /// thread.
    ///
    /// The tuple fields are, in order: requested backend (`None` means
    /// automatic routing), actual selected backend, operation count, route
    /// nanoseconds, validation nanoseconds, backend nanoseconds, optional
    /// dispatch count, fallback reason, optional resource counters, and
    /// resize-coefficient cache counters.
    pub fn take_pipeline_telemetry() -> Option<PipelineTelemetrySample> {
        take_pipeline_telemetry()
    }

    /// Takes operation-level backend-path evidence for the most recent
    /// managed pipeline sample on this thread.
    pub fn take_pipeline_operation_telemetry() -> Vec<PipelineOperationTelemetry> {
        take_pipeline_operation_telemetry()
    }

    /// Enables or disables the bounded GPU shader-dispatch coverage collector.
    ///
    /// The collector records which embedded WGSL shader variants actually
    /// dispatch during a managed parity run. It is disabled by default so
    /// ordinary image execution does not pay for a process-global map or a
    /// mutex on every GPU dispatch.
    pub fn set_gpu_shader_coverage_enabled(enabled: bool) -> bool {
        set_gpu_shader_coverage_enabled(enabled)
    }

    /// Takes and clears the GPU shader-dispatch coverage collected by the
    /// current process.
    ///
    /// This is execution coverage, not WGSL source line or branch coverage.
    /// The managed runner combines these records with the checked-in shader
    /// inventory and reports source instrumentation as a separate status.
    pub fn take_gpu_shader_coverage() -> Vec<GpuShaderDispatchTelemetry> {
        take_gpu_shader_coverage()
    }
}

// ── BackendImpl trait ──────────────────────────────────────────────────────

/// Implementation contract for a compute backend.
///
/// Backends advertise operation support before execution so the router can keep
/// a pipeline on one backend. Implementations should return errors instead of
/// panicking when runtime resources are unavailable.
pub(crate) trait BackendImpl: Send + Sync {
    /// Returns the backend identity used for routing and enable/disable state.
    fn name(&self) -> Backend;
    /// Returns backend preference. Larger values are selected first.
    fn priority(&self) -> u8; // higher = preferred
    /// Returns whether this backend can execute one pipeline operation.
    fn supports(&self, op: &PipelineOp) -> Result<bool, PilError>;
    /// Executes a sequence of operations against one image buffer.
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;
    /// Executes a batch with the context selected at the routing boundary.
    ///
    /// Backends that have not adopted context-driven strategy selection keep
    /// their existing executor through this default. CPU overrides it so each
    /// operation consumes the same injected backend and architecture choice.
    fn execute_batch_with_context(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        _context: &ExecutionContext,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(ops, img, mode)
    }
    /// Executes a context-routed batch with wrapper-owned palette inputs.
    ///
    /// Most backends do not consume palette metadata and retain their current
    /// executor. CPU overrides this boundary so exact P/PA strategies receive
    /// the palette resolved by the image wrapper.
    fn execute_batch_with_context_and_palette(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        context: &ExecutionContext,
        palette: registry::support::PaletteInput<'_>,
    ) -> Result<DynamicImage, PilError> {
        let _ = palette;
        self.execute_batch_with_context(ops, img, mode, context)
    }
    /// Executes a sequence without allowing the backend to recover through a
    /// different implementation.  Explicit backend locks use this boundary
    /// for capability audits; automatic routing continues to use
    /// [`Self::execute_batch`] and may select the CPU fallback.
    fn execute_batch_strict(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(ops, img, mode)
    }
    /// Returns a backend-native dispatch count when it can be reported without
    /// estimating or instrumenting the backend. Most backends currently do not
    /// expose this distinction, so the default is `None`.
    fn dispatch_count(&self, _ops: &[PipelineOp]) -> Option<u64> {
        None
    }
}

// ── Modules ────────────────────────────────────────────────────────────────

mod pool_cpu;
#[cfg(feature = "gpu")]
mod pool_gpu;
#[cfg(feature = "gpu")]
pub use pool_gpu::stream::{
    BatchError, BatchErrorKind, BatchKey, BatchOutput, BatchResult, GpuBatchConfig,
    GpuBatchExecutor, GpuBatchStats, GpuImage,
};
pub(crate) mod registry;

pub(crate) use pool_cpu::CpuPool;
pub(crate) use pool_cpu::ops::analysis::ops::{
    ProjectionOutput, execute_cmyk8_in_context as cpu_cmyk_projection_in_context,
    execute_rgb8_in_context as cpu_rgb_projection_in_context,
    prepare as prepare_cpu_getprojection,
};
#[cfg(feature = "gpu")]
pub(crate) use pool_gpu::GpuPool;
#[cfg(all(feature = "gpu", feature = "test-api"))]
pub(crate) fn arm_gpu_batch_admission_failure() {
    pool_gpu::stream::arm_test_admission_failure();
}

#[cfg(all(feature = "gpu", feature = "test-api"))]
pub(crate) fn clear_gpu_batch_test_fault() {
    pool_gpu::stream::clear_test_admission_failure();
}

#[cfg(test)]
mod tests {
    use super::{
        Architecture, Backend, execute_eager_cpu_arch_bytes, execute_prepared,
        prepare_execution,
    };
    use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
    use crate::raster::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn pipeline_telemetry_switch_is_thread_local() {
        let previous = Backend::set_pipeline_telemetry_enabled(true);

        assert!(Backend::pipeline_telemetry_enabled());
        let worker_enabled = std::thread::spawn(Backend::pipeline_telemetry_enabled)
            .join()
            .expect("telemetry state query thread");
        assert!(!worker_enabled);
        assert!(Backend::pipeline_telemetry_enabled());

        Backend::set_pipeline_telemetry_enabled(previous);
    }

    #[test]
    fn eager_cpu_arch_bytes_records_strategy_and_output_buffer_receipt() {
        let raw: Vec<u8> = (0..37 * 3)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(37, 1, raw).expect("RGB source shape must be valid"),
        );
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let mut output = vec![0; 37];
        let operation = PipelineOp::ExtractBand { index: 1 };
        let result = execute_eager_cpu_arch_bytes(
            "GetData",
            &image,
            Backend::Simd,
            &mut output,
            |context, output| {
                let prepared = crate::compute::registry::prepare_cpu(
                    &operation,
                    crate::compute::registry::support::OperationInputs {
                        image: &image,
                        palette: crate::compute::registry::support::PaletteInput::default(),
                    },
                    context,
                )?;
                let selection = *prepared.selection();
                let output_work = prepared
                    .execute_into(output)?
                    .expect("RGB ExtractBand has a caller-owned output strategy");
                Ok(Some((selection, output_work)))
            },
        );
        let receipt = Backend::take_pipeline_telemetry()
            .expect("CPU architecture execution receipt");
        let operations = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        result
            .expect("eager CPU architecture ExtractBand must succeed")
            .expect("RGB ExtractBand has a caller-owned output strategy");
        assert_eq!(
            output.as_slice(),
            image
                .as_bytes()
                .chunks_exact(3)
                .map(|pixel| pixel[1])
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(receipt.0, Some(Backend::Simd));
        assert_eq!(receipt.1, Backend::Cpu);
        assert_eq!(receipt.2, 1);
        let resource = receipt.8.expect("CPU architecture host-buffer receipt");
        assert_eq!(resource.host_allocation_count, 0);
        assert_eq!(resource.host_allocated_bytes, 0);
        let operation = operations
            .first()
            .expect("CPU architecture ExtractBand operation receipt");
        assert!(operation.selected_architecture.is_some());
        if operation.selected_architecture == Some(Architecture::Default) {
            assert_eq!(operation.path, "cpu");
            assert_eq!(operation.vector_block_count, 0);
        } else {
            assert!(operation.vector_block_count > 0);
            assert_eq!(operation.scalar_tail_count, 5);
            assert!(matches!(
                operation.path,
                "neon-deinterleave" | "ssse3-gather" | "vector-gather"
            ));
        }
    }

    #[test]
    fn simd_compatibility_request_uses_cpu_architecture_context() {
        let source = DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 2, Rgb([11, 22, 33])));
        let ops = [
            PipelineOp::PutPixel {
                x: 0,
                y: 0,
                color: (101, 102, 103, 255),
                palette_index: false,
            },
            PipelineOp::Transform {
                w: 2,
                h: 2,
                method: TransformMethod::Perspective,
                data: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0].into(),
                filter: ResampleFilter::Bilinear,
                fill: Some((0, 0, 0, 255)),
                fill_is_none: false,
                palette_fill: None,
            },
        ];

        let prepared = prepare_execution(&ops, Some(Backend::Simd))
            .expect("SIMD compatibility request should prepare as CPU architecture routing");
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let result = execute_prepared(&prepared, &ops, &source, Some("RGB"))
            .expect("CPU architecture strategies should execute the pipeline");
        let receipt = Backend::take_pipeline_telemetry()
            .expect("CPU architecture pipeline receipt");
        Backend::set_pipeline_telemetry_enabled(previous);

        assert_eq!(result.dimensions(), source.dimensions());
        assert_eq!(receipt.0, Some(Backend::Simd));
        assert_eq!(receipt.1, Backend::Cpu);
        assert_eq!(receipt.2, 2);
    }
}
