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
use crate::raster::{DynamicImage, Mode};

#[path = "backend_state.rs"]
mod backend_state;
pub(crate) mod eager;
mod execution;
mod execution_context;
pub(crate) mod helpers;
pub use execution_context::{Architecture, ExecutionContext};
#[cfg(feature = "test-api")]
pub(crate) fn set_test_cpu_architecture(architecture: Architecture) {
    execution_context::set_test_architecture(architecture);
}
#[path = "phase_telemetry.rs"]
mod phase_telemetry;
#[path = "telemetry.rs"]
mod telemetry;

#[doc(hidden)]
pub use telemetry::PipelineTelemetryOrigin;
pub use telemetry::{
    GpuShaderDispatchTelemetry, PipelineAllocationTelemetry, PipelineOperationTelemetry,
    PipelineResourceTelemetry,
};
pub(crate) use telemetry::{
    PipelineOperationTelemetryCapture, RestorePipelineOperationOrigins,
    account_host_buffer_boundary, begin_eager_cpu_operation_telemetry,
    begin_pipeline_operation_telemetry, cancel_eager_cpu_operation_telemetry,
    capture_active_origin, clear_pipeline_operation_architecture, enter_operation_origins,
    finish_eager_cpu_operation_telemetry, finish_eager_cpu_operation_telemetry_with_selection,
    finish_pipeline_operation_telemetry, host_resource_telemetry,
    record_eager_cpu_operation_receipt_with_selection,
    record_eager_cpu_operation_receipt_with_selection_and_receipts, record_pipeline_allocation,
    record_pipeline_backend_fallback, record_pipeline_operation_handoff,
    record_pipeline_operation_mode_conversion, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_selection,
    record_pipeline_operation_unsupported, record_pipeline_operation_vector_blocks,
    record_pipeline_resize_coeff_cache_hit, record_pipeline_resize_coeff_cache_miss,
    record_pipeline_resource_telemetry, reset_pipeline_allocation_telemetry,
    reset_pipeline_operation_telemetry, take_pipeline_allocation_telemetry,
    take_pipeline_operation_telemetry,
};
use telemetry::{
    PipelineTelemetry, elapsed_ns, pipeline_telemetry_enabled, pipeline_timestamp,
    record_pipeline_telemetry, set_gpu_shader_coverage_enabled, set_pipeline_telemetry_enabled,
    take_gpu_shader_coverage, take_pipeline_backend_override, take_pipeline_dispatch_count,
    take_pipeline_resize_coeff_cache_stats, take_pipeline_resource_telemetry,
    take_pipeline_telemetry,
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
    execute_eager_cpu_arch_bytes_with_context, execute_eager_cpu_arch_image_operation_with_context,
    execute_eager_cpu_arch_image_operation_with_context_and_receipts,
    execute_eager_cpu_getprojection, execute_eager_cpu_getprojection_with_receipts,
    execute_eager_cpu_linear_gradient, execute_eager_cpu_mandelbrot,
    execute_eager_cpu_radial_gradient, execute_eager_native_copy_result,
    execute_eager_native_copy_result_with_receipts, execute_eager_native_identity_result,
    execute_eager_native_identity_result_with_receipts,
};

#[cfg(feature = "gpu")]
pub(crate) use eager::{
    execute_eager_gpu_rgb_getprojection, gpu_cmyk_projection_terminal_supported,
    gpu_rgb_projection_supported, gpu_rgb_projection_terminal_supported,
};
pub(crate) use execution::{
    PreparedExecution, execute_prepared, execute_prepared_cpu_arch_convert_output,
    execute_prepared_cpu_arch_operation, execute_prepared_cpu_arch_terminal,
    execute_prepared_cpu_terminal, execute_prepared_shared_with_palette,
    execute_prepared_with_palette, prepare_execution,
};
#[cfg(feature = "gpu")]
pub(crate) use execution::{
    execute_prepared_gpu_cmyk_projection_terminal, execute_prepared_gpu_rgb_projection_terminal,
    merge_pipeline_resource_telemetry,
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

/// Identity of the WGPU adapter selected for GPU execution.
///
/// This diagnostic type is used by parity and benchmark receipts to attribute
/// GPU results to the physical device that executed them.
#[cfg(feature = "gpu")]
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAdapterInfo {
    /// Adapter index in the WGPU instance inventory when explicitly selected.
    pub selection_index: Option<usize>,
    /// WGPU adapter name, usually including the GPU model.
    pub name: String,
    /// PCI vendor identifier reported by WGPU.
    pub vendor: u32,
    /// PCI device identifier reported by WGPU.
    pub device: u32,
    /// WGPU adapter device type.
    pub device_type: String,
    /// WGPU graphics API backend used by the adapter.
    pub backend: String,
    /// Adapter driver name reported by WGPU.
    pub driver: String,
    /// Adapter driver details reported by WGPU.
    pub driver_info: String,
}

/// Return the selected WGPU adapter after GPU initialization, if one executed.
///
/// The function does not initialize a device and is intended for execution
/// receipts and benchmark attribution.
#[cfg(feature = "gpu")]
#[doc(hidden)]
pub fn selected_gpu_adapter_info() -> Option<GpuAdapterInfo> {
    pool_gpu::GpuPool::selected_adapter_info()
}

/// Enumerate WGPU adapters without creating a device.
#[cfg(feature = "gpu")]
#[doc(hidden)]
pub fn gpu_adapter_inventory() -> Vec<GpuAdapterInfo> {
    pool_gpu::GpuPool::adapter_inventory()
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
    /// resize-coefficient cache counters. For eager terminal operations over
    /// lazy images or masks, the sample measures the terminal operation only;
    /// source graph operation paths and selections are retained separately by
    /// [`Backend::take_pipeline_operation_telemetry`]. It does not aggregate
    /// source or mask graph timing and resource counters.
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
    /// `logical_mode` retains typed wrapper context for operations that need
    /// the source interpretation; CPU strategy selection reads `img.mode()`.
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        logical_mode: Option<Mode>,
    ) -> Result<DynamicImage, PilError>;
    /// Executes a batch with the context selected at the routing boundary.
    ///
    /// Backends that have not adopted context-driven strategy selection keep
    /// their existing executor through this default. CPU overrides it so each
    /// operation consumes the same injected backend and architecture choice.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn execute_batch_with_context(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        logical_mode: Option<Mode>,
        _context: &ExecutionContext,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(ops, img, logical_mode)
    }
    /// Executes a context-routed batch with wrapper-owned palette inputs.
    ///
    /// Most backends do not consume palette metadata and retain their current
    /// executor. CPU overrides this boundary so exact P/PA strategies receive
    /// the palette resolved by the image wrapper.
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn execute_batch_with_context_and_palette(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        logical_mode: Option<Mode>,
        context: &ExecutionContext,
        palette: registry::support::PaletteInput<'_>,
    ) -> Result<DynamicImage, PilError> {
        let _ = palette;
        self.execute_batch_with_context(ops, img, logical_mode, context)
    }
    /// Executes a sequence without allowing the backend to recover through a
    /// different implementation.  Explicit backend locks use this boundary
    /// for capability audits; automatic routing continues to use
    /// [`Self::execute_batch`] and may select the CPU fallback.
    fn execute_batch_strict(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        logical_mode: Option<Mode>,
    ) -> Result<DynamicImage, PilError> {
        self.execute_batch(ops, img, logical_mode)
    }
    /// Returns a backend-native dispatch count when it can be reported without
    /// estimating or instrumenting the backend. Most backends currently do not
    /// expose this distinction, so the default is `None`.
    fn dispatch_count(&self, _ops: &[PipelineOp]) -> Option<u64> {
        None
    }
}

// ── Modules ────────────────────────────────────────────────────────────────

pub(crate) mod pool_cpu;
#[cfg(feature = "gpu")]
mod pool_gpu;
#[cfg(feature = "gpu")]
pub use pool_gpu::stream::{
    BatchError, BatchErrorKind, BatchKey, BatchOutput, BatchResult, GpuBatchConfig,
    GpuBatchExecutor, GpuBatchStats, GpuImage,
};
pub(crate) mod registry;
pub use registry::support::{
    BandIndexRule, DrawRequestKind, DrawTextMaskKind, ExpandFillRule, F32RankCompatibility,
    FallbackReason, ImplementationId, ImplementationSupport, InputContract, MergeBandModeRule,
    ModeChannelCapacity, ModeSupport, OperationId, OperationModeRoute, OperationModeTransition,
    OperationRouteGuard, OperationRouteOutcome, OperationRoutePath, OperationRouteStage,
    OperationSupport, OperationTargetComposition, OperationTargetCompositionKind,
    OperationTransitionSource, OperationValueResult, OutputModeRule, PadFillRule, PaletteKnowledge,
    PasteInputRules, PasteMaskRule, PayloadOutput, PixelValueShape, PixelWriteRunFacts,
    RejectionKind, RequestFacts, SecondaryInputFacts, StrategyGuard, StrategyInputs,
    StrategySelection, SupportAssessment, SupportScope, UnresolvedGuard,
};

pub(crate) use pool_cpu::CpuPool;
pub(crate) use pool_cpu::ops::analysis::ops::{
    execute_cmyk8_selected as cpu_cmyk_projection_selected,
    execute_rgb8_selected as cpu_rgb_projection_selected, prepare as prepare_cpu_getprojection,
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
