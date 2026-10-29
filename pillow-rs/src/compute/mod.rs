//! Compute backend routing for deferred image pipelines.
//!
//! This module decides where a materialized [`crate::pipeline::PipelineOp`]
//! batch executes. The public surface is intentionally small: callers can
//! inspect compiled backends, enable or disable automatic routing choices, lock
//! an image pipeline to one backend, and execute a selected backend.
//!
//! # Routing Contract
//!
//! Automatic materialization may execute contiguous operation segments on the
//! best available backend. CPU is always present and acts as the universal
//! fallback. GPU and SIMD are selected only when they are active and every
//! operation in the segment reports contextual support. CPU and SIMD both
//! operate on host-resident image buffers, so a CPU↔SIMD handoff is a planner
//! event, not a device copy or a Python-level materialization. A GPU segment
//! performs its device transfer once, records the eligible dispatches, and
//! reads the final result back once; GPU/host segmentation must account for
//! those boundaries separately.
//!
//! # Adding Operations
//!
//! New operations must define the pipeline variant, CPU implementation, registry
//! key, and optional GPU/SIMD support together. The registry module is the
//! source of truth used by the routing and execution phases.

use crate::error::PilError;
use crate::image::Mode;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

#[path = "automatic_segments.rs"]
mod automatic_segments;
#[path = "backend_state.rs"]
mod backend_state;
mod eager;
mod execution;
mod execution_context;
#[path = "phase_telemetry.rs"]
mod phase_telemetry;
#[path = "telemetry.rs"]
mod telemetry;
#[path = "thumbnail_fusion.rs"]
mod thumbnail_fusion;
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
    execute_eager_simd_bytes, finish_eager_cpu_operation_telemetry,
    finish_pipeline_operation_telemetry, host_resource_telemetry,
    record_eager_cpu_operation_receipt, record_eager_cpu_operation_receipt_with_reason,
    record_pipeline_allocation, record_pipeline_backend_fallback,
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
    cpu_is_only_active_backend, preferred_active_backend, simd_is_only_active_backend,
};
pub(crate) use eager::{
    begin_eager_cpu_mandelbrot_telemetry, execute_eager_cpu_getprojection,
    execute_eager_cpu_image_operation, execute_eager_cpu_shared_result,
    execute_eager_simd_getprojection, execute_eager_simd_image_operation,
    finish_eager_cpu_mandelbrot_telemetry, simd_cmyk_projection,
    simd_cmyk_projection_from_positive_writes, simd_rgb_projection, simd_unsharp_blend_rgb_150,
    try_simd_linear_gradient, try_simd_mandelbrot, try_simd_radial_gradient,
};
#[cfg(feature = "gpu")]
pub(crate) use eager::{
    execute_eager_gpu_rgb_getprojection, gpu_cmyk_projection_terminal_supported,
    gpu_rgb_projection_supported, gpu_rgb_projection_terminal_supported,
};
#[cfg(test)]
use execution::execute_automatic_simd_segments;
pub(crate) use execution::{
    PreparedExecution, execute_prepared, execute_prepared_cpu_terminal, execute_prepared_shared,
    execute_prepared_simd_terminal, prepare_execution,
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
pub(crate) use thumbnail_fusion::{
    cpu_rgb_putpixel_thumbnail_fusion_supported, simd_rgb_putpixel_thumbnail_fusion_supported,
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
/// Backends are ordered by preference, with CPU as the universal fallback.
/// Callers can enable or disable backends globally, or request one explicitly
/// when routing a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Backend {
    /// Scalar CPU implementation. This backend is always available.
    Cpu = 0,
    /// WebGPU/wgpu implementation when the `gpu` feature is enabled.
    Gpu = 1,
    /// SIMD implementation for operations with vectorized adapters.
    Simd = 2,
    /// Rayon-backed CPU implementation when the `parallel` feature is enabled.
    ParallelCpu = 3,
}

pub(crate) use execution_context::{Architecture, ExecutionContext};

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
    /// Accepted values are `"cpu"`, `"gpu"`, `"simd"`, and
    /// `"parallel-cpu"`, case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            "simd" => Some(Self::Simd),
            "parallel-cpu" | "parallel_cpu" => Some(Self::ParallelCpu),
            _ => None,
        }
    }

    /// Return the priority used by automatic backend activation order.
    pub(crate) const fn priority(self) -> u8 {
        match self {
            Self::Cpu => 0,
            Self::ParallelCpu => 25,
            Self::Simd => 50,
            Self::Gpu => 100,
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
    fn supports(&self, context: &ExecutionContext, op: &PipelineOp) -> Result<bool, PilError>;
    /// Returns whether this backend can execute `op` for this concrete image
    /// and logical mode.  Operation-only registry support is deliberately not
    /// enough for backends whose implementation depends on layout, mode, or
    /// parameters.  Backends that do not have contextual restrictions inherit
    /// the operation-only answer.
    fn supports_for_image(
        &self,
        context: &ExecutionContext,
        op: &PipelineOp,
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<bool, PilError> {
        let _ = (img, mode);
        self.supports(context, op)
    }
    /// Executes a sequence of operations against one image buffer.
    fn execute_batch(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;
    /// Executes with the wrapper's precomputed mode selector.
    ///
    /// Backends still awaiting typed-mode migration pass through an explicit
    /// storage-facing compatibility spelling here. CPU overrides this method
    /// and retains the selector through each operation in the batch.
    fn execute_batch_with_mode(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Mode,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(context, ops, img, Some(mode.compatibility_name()))
    }
    /// Executes a strict backend batch with the wrapper's typed mode.
    fn execute_batch_strict_with_mode(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Mode,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch_strict(context, ops, img, Some(mode.compatibility_name()))
    }
    /// Executes a sequence without allowing the backend to recover through a
    /// different implementation.  Explicit backend locks use this boundary
    /// for capability audits; automatic routing continues to use
    /// [`Self::execute_batch`] and may select the CPU fallback.
    fn execute_batch_strict(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(context, ops, img, mode)
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
mod pool_simd;
pub(crate) use pool_simd::ops::adapters::{
    simd_extract_band_into, simd_extract_band_into_in_context, simd_grayscale_with_evidence,
    simd_grayscale_with_evidence_in_context,
};

pub(crate) mod registry;

pub(crate) use pool_cpu::CpuPool;
#[cfg(feature = "gpu")]
pub(crate) use pool_gpu::GpuPool;
pub(crate) use pool_simd::SimdPool;

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
        Backend, ExecutionContext, PilError, execute_automatic_simd_segments,
        execute_eager_simd_image_operation, simd_extract_band_into_in_context,
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
    fn eager_simd_image_operation_records_vector_and_allocation_evidence() {
        let raw: Vec<u8> = (0..37 * 3)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(37, 1, raw).expect("RGB source shape must be valid"),
        );
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        let context = ExecutionContext::for_backend(Backend::Simd);
        let result = execute_eager_simd_image_operation(&context, "ExtractBand", &image, || {
            let mut output = crate::checked_dims::CheckedDims::new(37, 1, 1)?.alloc_buffer();
            let (vector_blocks, scalar_tail, path) =
                simd_extract_band_into_in_context(&context, &image, 1, Some("RGB"), &mut output)?;
            let output = crate::raster::GrayImage::from_raw(37, 1, output).ok_or_else(|| {
                PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into())
            })?;
            Ok((
                DynamicImage::ImageLuma8(output),
                vector_blocks,
                scalar_tail,
                path,
            ))
        });
        let receipt = Backend::take_pipeline_telemetry().expect("SIMD execution receipt");
        let operations = Backend::take_pipeline_operation_telemetry();
        Backend::set_pipeline_telemetry_enabled(previous);

        let result = result.expect("eager SIMD ExtractBand must succeed");
        assert_eq!(
            result.as_bytes(),
            image
                .as_bytes()
                .chunks_exact(3)
                .map(|pixel| pixel[1])
                .collect::<Vec<_>>()
                .as_slice()
        );
        assert_eq!(receipt.0, Some(Backend::Simd));
        assert_eq!(receipt.1, Backend::Simd);
        assert_eq!(receipt.2, 1);
        assert_eq!(receipt.7, None);
        let resource = receipt.8.expect("SIMD host-buffer receipt");
        assert_eq!(resource.host_allocation_count, 1);
        assert_eq!(resource.host_allocated_bytes, 37);
        let operation = operations
            .first()
            .expect("SIMD ExtractBand operation receipt");
        assert!(operation.vector_block_count > 0);
        assert_eq!(operation.scalar_tail_count, 5);
        assert!(matches!(
            operation.path,
            "neon-deinterleave" | "ssse3-gather" | "vector-gather"
        ));
    }

    #[test]
    fn automatic_simd_receipt_uses_final_cpu_segment() {
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

        let (_result, actual_backend, fallback_reason, _resource) =
            execute_automatic_simd_segments(
                &ExecutionContext::for_backend(Backend::Simd),
                &ops,
                &source,
                Some("RGB"),
            )
            .expect("mixed SIMD/CPU pipeline should execute");

        assert_eq!(actual_backend, Backend::Cpu);
        assert_eq!(
            fallback_reason.as_deref(),
            Some("exact host semantic control: SIMD image-layout guard for Transform")
        );
    }
}
