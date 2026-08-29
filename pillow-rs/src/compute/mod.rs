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
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

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
}

impl Backend {
    /// Parses a backend name used by CLI, environment, or binding-layer options.
    ///
    /// Accepted values are `"cpu"`, `"gpu"`, and `"simd"`, case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            "simd" => Some(Self::Simd),
            _ => None,
        }
    }

    /// Enables or disables bounded pipeline execution telemetry.
    ///
    /// Telemetry is disabled by default. The return value is the previous
    /// setting. When enabled, one completed sample is retained per thread;
    /// recording a new sample replaces that thread's previous sample.
    pub fn set_pipeline_telemetry_enabled(enabled: bool) -> bool {
        set_pipeline_telemetry_enabled(enabled)
    }

    /// Returns whether pipeline execution telemetry is enabled.
    pub fn pipeline_telemetry_enabled() -> bool {
        pipeline_telemetry_enabled()
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
    pub fn take_pipeline_telemetry() -> Option<(
        Option<Self>,
        Self,
        usize,
        u64,
        u64,
        u64,
        Option<u64>,
        Option<String>,
        Option<PipelineResourceTelemetry>,
        u64,
        u64,
    )> {
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

/// One embedded WGSL shader variant observed at least once by a GPU dispatch.
///
/// `shader_file` is the checked-in WGSL asset name. `variant_name` identifies
/// the public registry key or an internal multi-pass/fusion variant. Keeping
/// both fields makes the report useful when several dispatch variants share a
/// source file.
#[derive(Debug, Clone, Copy)]
pub struct GpuShaderDispatchTelemetry {
    /// Public registry key or internal pipeline variant.
    pub variant_name: &'static str,
    /// Checked-in WGSL file name.
    pub shader_file: &'static str,
    /// Number of dispatch commands encoded for this variant.
    pub dispatches: u64,
    /// Total workgroups submitted for this variant.
    pub workgroups: u64,
}

#[derive(Debug, Default)]
struct GpuShaderDispatchCounters {
    dispatches: u64,
    workgroups: u64,
}

/// Resource counters attached to one completed backend execution sample.
///
/// These counters are deliberately optional at the receipt boundary: the GPU
/// path can observe its transfer and retained-buffer sizes, while CPU/SIMD
/// allocations are not globally instrumented by the benchmark collector yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineResourceTelemetry {
    /// Host bytes written to the backend input resource.
    pub upload_bytes: u64,
    /// Bytes copied from the backend result into host-visible staging.
    pub readback_bytes: u64,
    /// Padded bytes written for secondary images and LUT resources.
    pub auxiliary_bytes: u64,
    /// Padded bytes written for per-operation parameters.
    pub parameter_bytes: u64,
    /// Bytes retained by the reusable GPU working-set pool after completion.
    pub retained_cache_bytes: u64,
    /// Number of explicitly recorded full-frame backend/device copies.
    /// CPU/SIMD output allocation is not inferred as a copy from equal-sized
    /// buffers; their host handoffs are represented separately by operation
    /// telemetry.
    pub full_frame_copy_count: u64,
    /// Number of logical mode-widening conversions before backend execution.
    pub mode_conversion_count: u64,
    /// Number of owned host output-buffer boundaries observed by CPU/SIMD.
    /// This is intentionally not a process-global allocator count.
    pub host_buffer_count: u64,
    /// Sum of the owned host output-buffer sizes observed at those boundaries.
    pub host_buffer_bytes: u64,
    /// Maximum simultaneously live input/output host bytes at a boundary.
    pub peak_live_host_bytes: u64,
    /// Number of public operations executed through a fused backend path.
    /// This is zero for an ordinary unfused batch and is not a correctness
    /// or dispatch-count estimate.
    pub fused_operation_count: u64,
    /// Number of checked host pixel buffers allocated during execution.
    pub host_allocation_count: u64,
    /// Sum of checked host pixel-buffer sizes allocated during execution.
    pub host_allocated_bytes: u64,
}

/// Evidence for one public operation inside a backend execution.
///
/// `path` is intentionally a small controlled vocabulary.  `vector` means a
/// vector kernel processed at least one block; `native-copy` is for bandwidth
/// operations such as crop; `scalar-control` is the scalar part of a valid
/// SIMD-capable operation (geometry, validation, or a tail); `cpu` means the
/// CPU backend produced the operation; and `unsupported` means strict SIMD
/// rejected it during contextual preflight.
#[derive(Debug, Clone, Copy)]
pub struct PipelineOperationTelemetry {
    /// Registry key for the public pipeline operation.
    pub operation: &'static str,
    /// Controlled execution-path classification.
    pub path: &'static str,
    /// Number of vector blocks processed by the operation.
    pub vector_block_count: u64,
    /// Number of scalar tail elements processed after vector blocks.
    pub scalar_tail_count: u64,
    /// Number of logical mode conversions attributable to this operation.
    pub mode_conversion_count: u64,
    /// Number of CPU↔SIMD handoffs attributable to this operation.
    pub handoff_count: u64,
}

#[derive(Debug, Clone, Copy)]
struct PipelineOperationSample {
    operation: &'static str,
    path: &'static str,
    vector_block_count: u64,
    scalar_tail_count: u64,
    mode_conversion_count: u64,
    handoff_count: u64,
}

/// Safe, thread-local allocation counters for checked host pixel buffers.
///
/// This intentionally does not replace a process allocator profiler. It
/// observes allocations at the repository's checked pixel-buffer boundary so
/// benchmark receipts can distinguish known image-buffer ownership from
/// unobserved allocator activity in dependencies and bindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineAllocationTelemetry {
    /// Number of checked host pixel buffers allocated.
    pub allocation_count: u64,
    /// Sum of checked host pixel-buffer sizes.
    pub allocated_bytes: u64,
}

/// Start bounded host-buffer telemetry for a backend execution.
pub(crate) fn host_resource_telemetry(img: &DynamicImage) -> PipelineResourceTelemetry {
    let bytes = img.as_bytes().len() as u64;
    PipelineResourceTelemetry {
        host_buffer_count: u64::from(bytes != 0),
        host_buffer_bytes: bytes,
        peak_live_host_bytes: bytes,
        ..PipelineResourceTelemetry::default()
    }
}

/// Account for one owned output buffer crossing a backend operation boundary.
/// Internal allocator activity inside a kernel remains deliberately outside
/// this receipt; the counters describe observable working-buffer ownership.
pub(crate) fn account_host_buffer_boundary(
    telemetry: &mut PipelineResourceTelemetry,
    before: &DynamicImage,
    after: &DynamicImage,
) {
    let before_bytes = before.as_bytes().len() as u64;
    let after_bytes = after.as_bytes().len() as u64;
    telemetry.host_buffer_count = telemetry.host_buffer_count.saturating_add(1);
    telemetry.host_buffer_bytes = telemetry.host_buffer_bytes.saturating_add(after_bytes);
    telemetry.peak_live_host_bytes = telemetry
        .peak_live_host_bytes
        .max(before_bytes.saturating_add(after_bytes));
    record_pipeline_operation_mode_conversion(u64::from(before.color() != after.color()));
    if before.color() != after.color() {
        telemetry.mode_conversion_count = telemetry.mode_conversion_count.saturating_add(1);
    }
}

#[derive(Debug, Clone)]
struct PipelineTelemetry {
    requested_backend: Option<Backend>,
    actual_backend: Backend,
    operation_count: usize,
    route_ns: u64,
    validation_ns: u64,
    backend_ns: u64,
    dispatch_count: Option<u64>,
    fallback_reason: Option<String>,
    resource: Option<PipelineResourceTelemetry>,
    resize_coeff_cache_hits: u64,
    resize_coeff_cache_misses: u64,
}

static PIPELINE_TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);
static GPU_SHADER_COVERAGE_ENABLED: AtomicBool = AtomicBool::new(false);
static GPU_SHADER_COVERAGE: OnceLock<
    Mutex<BTreeMap<(&'static str, &'static str), GpuShaderDispatchCounters>>,
> = OnceLock::new();

thread_local! {
    static LAST_PIPELINE_TELEMETRY: RefCell<Option<PipelineTelemetry>> = const { RefCell::new(None) };
    static LAST_PIPELINE_RESOURCE_TELEMETRY: RefCell<Option<PipelineResourceTelemetry>> = const { RefCell::new(None) };
    static LAST_PIPELINE_BACKEND_OVERRIDE: RefCell<Option<(Backend, String)>> = const { RefCell::new(None) };
    static LAST_PIPELINE_DISPATCH_COUNT: RefCell<Option<u64>> = const { RefCell::new(None) };
    static LAST_PIPELINE_RESIZE_CACHE_STATS: RefCell<(u64, u64)> = const { RefCell::new((0, 0)) };
    static LAST_PIPELINE_ALLOCATION_TELEMETRY: RefCell<PipelineAllocationTelemetry> = const { RefCell::new(PipelineAllocationTelemetry { allocation_count: 0, allocated_bytes: 0 }) };
    static LAST_PIPELINE_OPERATION_TELEMETRY: RefCell<Vec<PipelineOperationSample>> = const { RefCell::new(Vec::new()) };
    static ACTIVE_PIPELINE_OPERATION_TELEMETRY: RefCell<Vec<PipelineOperationSample>> = const { RefCell::new(Vec::new()) };
    static PENDING_PIPELINE_HANDOFFS: RefCell<u64> = const { RefCell::new(0) };
}

fn set_pipeline_telemetry_enabled(enabled: bool) -> bool {
    let previous = PIPELINE_TELEMETRY_ENABLED.swap(enabled, Ordering::Relaxed);
    if !enabled {
        LAST_PIPELINE_TELEMETRY.with(|last| {
            *last.borrow_mut() = None;
        });
        LAST_PIPELINE_RESOURCE_TELEMETRY.with(|last| {
            *last.borrow_mut() = None;
        });
        LAST_PIPELINE_BACKEND_OVERRIDE.with(|last| {
            *last.borrow_mut() = None;
        });
        LAST_PIPELINE_DISPATCH_COUNT.with(|last| {
            *last.borrow_mut() = None;
        });
        LAST_PIPELINE_RESIZE_CACHE_STATS.with(|last| {
            *last.borrow_mut() = (0, 0);
        });
        LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
            last.borrow_mut().clear();
        });
        ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
            active.borrow_mut().clear();
        });
        PENDING_PIPELINE_HANDOFFS.with(|pending| {
            *pending.borrow_mut() = 0;
        });
        reset_pipeline_allocation_telemetry();
    }
    previous
}

fn set_gpu_shader_coverage_enabled(enabled: bool) -> bool {
    let previous = GPU_SHADER_COVERAGE_ENABLED.swap(enabled, Ordering::Relaxed);
    if !enabled {
        if let Some(coverage) = GPU_SHADER_COVERAGE.get()
            && let Ok(mut coverage) = coverage.lock()
        {
            coverage.clear();
        }
    }
    previous
}

/// Record one actual GPU dispatch for the managed shader execution report.
#[cfg(feature = "gpu")]
pub(crate) fn record_gpu_shader_dispatch(
    variant_name: &'static str,
    shader_file: &'static str,
    workgroups: u64,
) {
    if !GPU_SHADER_COVERAGE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let coverage = GPU_SHADER_COVERAGE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut coverage) = coverage.lock() else {
        return;
    };
    let counters = coverage.entry((variant_name, shader_file)).or_default();
    counters.dispatches = counters.dispatches.saturating_add(1);
    counters.workgroups = counters.workgroups.saturating_add(workgroups);
}

fn take_gpu_shader_coverage() -> Vec<GpuShaderDispatchTelemetry> {
    let Some(coverage) = GPU_SHADER_COVERAGE.get() else {
        return Vec::new();
    };
    let Ok(mut coverage) = coverage.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut *coverage)
        .into_iter()
        .map(
            |((variant_name, shader_file), counters)| GpuShaderDispatchTelemetry {
                variant_name,
                shader_file,
                dispatches: counters.dispatches,
                workgroups: counters.workgroups,
            },
        )
        .collect()
}

fn reset_pipeline_allocation_telemetry() {
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| {
        *last.borrow_mut() = PipelineAllocationTelemetry::default();
    });
}

fn take_pipeline_allocation_telemetry() -> PipelineAllocationTelemetry {
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Clears operation-level records before a new managed pipeline sample.
pub(crate) fn reset_pipeline_operation_telemetry() {
    LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
        last.borrow_mut().clear();
    });
    ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        active.borrow_mut().clear();
    });
    PENDING_PIPELINE_HANDOFFS.with(|pending| {
        *pending.borrow_mut() = 0;
    });
}

/// Takes operation-level execution evidence collected on this thread.
pub fn take_pipeline_operation_telemetry() -> Vec<PipelineOperationTelemetry> {
    LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
        last.borrow_mut()
            .drain(..)
            .map(|sample| PipelineOperationTelemetry {
                operation: sample.operation,
                path: sample.path,
                vector_block_count: sample.vector_block_count,
                scalar_tail_count: sample.scalar_tail_count,
                mode_conversion_count: sample.mode_conversion_count,
                handoff_count: sample.handoff_count,
            })
            .collect()
    })
}

/// Begin evidence collection for one logical operation.
pub(crate) fn begin_pipeline_operation_telemetry(operation: &'static str) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        active.borrow_mut().push(PipelineOperationSample {
            operation,
            // An adapter starts in scalar control until it proves that a
            // vector block or a native-copy path actually ran.
            path: "scalar-control",
            vector_block_count: 0,
            scalar_tail_count: 0,
            mode_conversion_count: 0,
            handoff_count: PENDING_PIPELINE_HANDOFFS.with(|pending| {
                std::mem::take(&mut *pending.borrow_mut())
            }),
        });
    });
}

/// Mark the active operation's data path.
pub(crate) fn record_pipeline_operation_path(path: &'static str) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        if let Some(sample) = active.borrow_mut().last_mut() {
            sample.path = path;
        }
    });
}

/// Add vector-block evidence to the active operation.
pub(crate) fn record_pipeline_operation_vector_blocks(count: u64) {
    if !pipeline_telemetry_enabled() || count == 0 {
        return;
    }
    ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        if let Some(sample) = active.borrow_mut().last_mut() {
            // A native-copy kernel may use vector loads/stores for bandwidth
            // without being an arithmetic vector kernel. Preserve that more
            // specific classification while still promoting ordinary
            // scalar-control samples to `vector` when their data plane proves
            // that at least one vector block ran.
            if sample.path != "native-copy" {
                sample.path = "vector";
            }
            sample.vector_block_count = sample.vector_block_count.saturating_add(count);
        }
    });
}

/// Add scalar-tail evidence to the active operation.
pub(crate) fn record_pipeline_operation_scalar_tail(count: u64) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        if let Some(sample) = active.borrow_mut().last_mut() {
            sample.scalar_tail_count = sample.scalar_tail_count.saturating_add(count);
        }
    });
}

/// Add an operation-local mode-conversion count.
pub(crate) fn record_pipeline_operation_mode_conversion(count: u64) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    let recorded_active = ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        let mut active = active.borrow_mut();
        if let Some(sample) = active.last_mut() {
            sample.mode_conversion_count = sample.mode_conversion_count.saturating_add(count);
            true
        } else {
            false
        }
    });
    if !recorded_active {
        LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
            if let Some(sample) = last.borrow_mut().last_mut() {
                sample.mode_conversion_count = sample.mode_conversion_count.saturating_add(count);
            }
        });
    }
}

/// Add an operation-local CPU↔SIMD handoff count.
pub(crate) fn record_pipeline_operation_handoff(count: u64) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    let recorded_active = ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| {
        if let Some(sample) = active.borrow_mut().last_mut() {
            sample.handoff_count = sample.handoff_count.saturating_add(count);
            true
        } else {
            false
        }
    });
    if !recorded_active {
        PENDING_PIPELINE_HANDOFFS.with(|pending| {
            let mut pending = pending.borrow_mut();
            *pending = pending.saturating_add(count);
        });
    }
}

/// Finish evidence collection for one logical operation.
pub(crate) fn finish_pipeline_operation_telemetry() {
    if !pipeline_telemetry_enabled() {
        return;
    }
    let Some(sample) = ACTIVE_PIPELINE_OPERATION_TELEMETRY.with(|active| active.borrow_mut().pop())
    else {
        return;
    };
    LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
        last.borrow_mut().push(sample);
    });
}

/// Record an operation rejected before backend execution.
pub(crate) fn record_pipeline_operation_unsupported(operation: &'static str) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_OPERATION_TELEMETRY.with(|last| {
        last.borrow_mut().push(PipelineOperationSample {
            operation,
            path: "unsupported",
            vector_block_count: 0,
            scalar_tail_count: 0,
            mode_conversion_count: 0,
            handoff_count: 0,
        });
    });
}

/// Records one checked host pixel-buffer allocation for the active managed
/// benchmark sample.
pub(crate) fn record_pipeline_allocation(bytes: usize) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| {
        let mut sample = last.borrow_mut();
        sample.allocation_count = sample.allocation_count.saturating_add(1);
        sample.allocated_bytes = sample.allocated_bytes.saturating_add(bytes as u64);
    });
}

fn pipeline_telemetry_enabled() -> bool {
    PIPELINE_TELEMETRY_ENABLED.load(Ordering::Relaxed)
}

fn take_pipeline_telemetry() -> Option<(
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
)> {
    LAST_PIPELINE_TELEMETRY.with(|last| {
        last.borrow_mut().take().map(|sample| {
            (
                sample.requested_backend,
                sample.actual_backend,
                sample.operation_count,
                sample.route_ns,
                sample.validation_ns,
                sample.backend_ns,
                sample.dispatch_count,
                sample.fallback_reason,
                sample.resource,
                sample.resize_coeff_cache_hits,
                sample.resize_coeff_cache_misses,
            )
        })
    })
}

/// Record a resize coefficient cache hit for the current timed pipeline.
pub(crate) fn record_pipeline_resize_coeff_cache_hit() {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_RESIZE_CACHE_STATS.with(|stats| {
        let mut stats = stats.borrow_mut();
        stats.0 = stats.0.saturating_add(1);
    });
}

/// Record a resize coefficient cache miss for the current timed pipeline.
pub(crate) fn record_pipeline_resize_coeff_cache_miss() {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_RESIZE_CACHE_STATS.with(|stats| {
        let mut stats = stats.borrow_mut();
        stats.1 = stats.1.saturating_add(1);
    });
}

fn take_pipeline_resize_coeff_cache_stats() -> (u64, u64) {
    LAST_PIPELINE_RESIZE_CACHE_STATS.with(|stats| std::mem::take(&mut *stats.borrow_mut()))
}

// This recorder is shared by CPU/SIMD and the optional GPU executor. The
// recorder remains a no-op unless benchmark telemetry is explicitly enabled.
pub(crate) fn record_pipeline_resource_telemetry(resource: PipelineResourceTelemetry) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_RESOURCE_TELEMETRY.with(|last| {
        *last.borrow_mut() = Some(resource);
    });
}

/// Record the dispatch count observed by a backend after its internal plan
/// rewrites.  The router cannot derive this for GPU point-op fusion because
/// it only sees the unfused public operation list.
#[cfg(feature = "gpu")]
pub(crate) fn record_pipeline_dispatch_count(count: u64) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_DISPATCH_COUNT.with(|last| {
        *last.borrow_mut() = Some(count);
    });
}

fn take_pipeline_dispatch_count() -> Option<u64> {
    LAST_PIPELINE_DISPATCH_COUNT.with(|last| last.borrow_mut().take())
}

fn take_pipeline_resource_telemetry() -> Option<PipelineResourceTelemetry> {
    LAST_PIPELINE_RESOURCE_TELEMETRY.with(|last| last.borrow_mut().take())
}

/// Record a backend-internal fallback so the public receipt reports the
/// executor that actually produced pixels, not only the router's request.
pub(crate) fn record_pipeline_backend_fallback(reason: impl Into<String>) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_BACKEND_OVERRIDE.with(|last| {
        *last.borrow_mut() = Some((Backend::Cpu, reason.into()));
    });
    record_pipeline_operation_handoff(1);
}

fn take_pipeline_backend_override() -> Option<(Backend, String)> {
    LAST_PIPELINE_BACKEND_OVERRIDE.with(|last| last.borrow_mut().take())
}

fn record_pipeline_telemetry(sample: PipelineTelemetry) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_TELEMETRY.with(|last| {
        *last.borrow_mut() = Some(sample);
    });
}

fn elapsed_ns(start: Option<Instant>) -> u64 {
    start
        .map(|start| start.elapsed().as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

// ``std::time::Instant`` is not implemented by the bare wasm32 target used
// by the browser package.  Backend identity and dispatch receipts remain
// useful there, so keep telemetry available while reporting zero for its
// host-clock fields instead of panicking during lazy materialization.
fn pipeline_timestamp() -> Option<Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(Instant::now())
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
    /// Returns whether this backend can execute `op` for this concrete image
    /// and logical mode.  Operation-only registry support is deliberately not
    /// enough for backends whose implementation depends on layout, mode, or
    /// parameters.  Backends that do not have contextual restrictions inherit
    /// the operation-only answer.
    fn supports_for_image(
        &self,
        op: &PipelineOp,
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<bool, PilError> {
        let _ = (img, mode);
        self.supports(op)
    }
    /// Executes a sequence of operations against one image buffer.
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;
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
mod pool_simd;
pub(crate) mod registry;

pub(crate) use pool_cpu::CpuPool;
#[cfg(feature = "gpu")]
pub(crate) use pool_gpu::GpuPool;
pub(crate) use pool_simd::SimdPool;

// ── Backend activation ─────────────────────────────────────────────────────

static ACTIVE: std::sync::OnceLock<Mutex<HashSet<Backend>>> = std::sync::OnceLock::new();

fn active() -> &'static Mutex<HashSet<Backend>> {
    ACTIVE.get_or_init(|| {
        let mut s = HashSet::new();
        s.insert(Backend::Cpu);
        Mutex::new(s)
    })
}

fn active_lock() -> Result<MutexGuard<'static, HashSet<Backend>>, PilError> {
    active()
        .lock()
        .map_err(|_| PilError::InternalError("compute backend state mutex poisoned".to_string()))
}

/// Enables a backend for future automatic routing.
///
/// Returns `true` when the backend was newly inserted into the active set.
///
/// # Errors
///
/// Returns [`PilError::InternalError`] if the global backend state has been
/// poisoned by a previous panic while it was being modified.
pub fn enable_backend(b: Backend) -> Result<bool, PilError> {
    Ok(active_lock()?.insert(b))
}
/// Disables a backend for future automatic routing.
///
/// Returns `true` when the backend was previously active.
///
/// # Errors
///
/// Returns [`PilError::InternalError`] if the global backend state has been
/// poisoned by a previous panic while it was being modified.
pub fn disable_backend(b: Backend) -> Result<bool, PilError> {
    Ok(active_lock()?.remove(&b))
}
/// Returns whether a backend is currently eligible for automatic routing.
///
/// # Errors
///
/// Returns [`PilError::InternalError`] if the global backend state has been
/// poisoned by a previous panic while it was being inspected.
pub fn backend_enabled(b: Backend) -> Result<bool, PilError> {
    Ok(active_lock()?.contains(&b))
}

/// Returns active backends ordered by routing preference.
///
/// # Errors
///
/// Returns [`PilError::InternalError`] if the global backend state has been
/// poisoned by a previous panic while it was being inspected.
pub fn active_backends() -> Result<Vec<Backend>, PilError> {
    let a = active_lock()?;
    let mut v: Vec<Backend> = a.iter().copied().collect();
    v.sort_by_key(|b| std::cmp::Reverse(*b as u8));
    Ok(v)
}

// ── Pool registry ──────────────────────────────────────────────────────────

fn pools() -> &'static [Box<dyn BackendImpl>] {
    static POOLS: OnceLock<Vec<Box<dyn BackendImpl>>> = OnceLock::new();
    POOLS.get_or_init(|| {
        let mut v: Vec<Box<dyn BackendImpl>> = vec![
            Box::new(CpuPool),
            #[cfg(feature = "gpu")]
            Box::new(GpuPool),
            Box::new(SimdPool),
        ];
        v.sort_by_key(|b| std::cmp::Reverse(b.priority()));
        v
    })
}

/// Returns every backend compiled into this crate.
pub fn available_backends() -> Vec<Backend> {
    pools().iter().map(|p| p.name()).collect()
}

/// Try the eager `Image.linear_gradient` constructor on the active SIMD
/// backend.
///
/// Eager module-level generators do not have a source image on which to hang
/// a deferred pipeline.  Keep their backend boundary here so strict SIMD
/// parity can observe the actual data-plane executor and so an unsupported
/// SIMD contract returns an error instead of silently entering the CPU loop.
pub(crate) fn try_simd_linear_gradient(mode: &str) -> Result<Option<DynamicImage>, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let active_set = active_lock()?.clone();
    let route_ns = elapsed_ns(route_start);
    if !active_set.contains(&Backend::Simd) {
        return Ok(None);
    }

    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let validation_start = timed.then(pipeline_timestamp).flatten();
    let requested_backend = (active_set.len() == 1).then_some(Backend::Simd);
    let validation_ns = elapsed_ns(validation_start);
    if timed {
        begin_pipeline_operation_telemetry("LinearGradient");
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = pool_simd::ops::adapters::simd_linear_gradient_generate(mode);
    let backend_ns = elapsed_ns(backend_start);

    if timed {
        if result.is_err() {
            record_pipeline_operation_path("unsupported");
        }
        finish_pipeline_operation_telemetry();

        let resource = result.as_ref().ok().map(|image| {
            let bytes = image.as_bytes().len() as u64;
            let allocation = take_pipeline_allocation_telemetry();
            PipelineResourceTelemetry {
                host_buffer_count: u64::from(bytes != 0),
                host_buffer_bytes: bytes,
                peak_live_host_bytes: bytes,
                host_allocation_count: allocation.allocation_count,
                host_allocated_bytes: allocation.allocated_bytes,
                ..PipelineResourceTelemetry::default()
            }
        });
        if let Some(resource) = resource {
            record_pipeline_resource_telemetry(resource);
        }
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend,
            actual_backend: Backend::Simd,
            operation_count: 1,
            route_ns,
            validation_ns,
            backend_ns,
            dispatch_count,
            fallback_reason: None,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }

    result.map(Some)
}

// ── Router ─────────────────────────────────────────────────────────────────

fn route_decision(
    ops: &[PipelineOp],
    explicit: Option<Backend>,
) -> Result<(Backend, Option<String>, bool), PilError> {
    if let Some(b) = explicit {
        // An explicit request must still cross the normal validation boundary;
        // callers may request an inactive or unsupported backend deliberately.
        return Ok((b, None, false));
    }
    // Snapshot activation policy before walking the operation list. Support
    // checks can inspect registry metadata and may grow as the descriptor
    // table expands; holding the global policy mutex across that scan would
    // serialize unrelated pipeline construction and backend toggles.
    let active_set = active_lock()?.clone();
    let mut fallback_reason = None;
    for pool in pools() {
        if active_set.contains(&pool.name()) {
            let mut supports_all = true;
            let mut supports_any = false;
            for op in ops {
                if pool.supports(op)? {
                    supports_any = true;
                } else {
                    supports_all = false;
                    if pool.name() != Backend::Cpu && fallback_reason.is_none() {
                        fallback_reason = Some(format!(
                            "{} does not support {}",
                            backend_label(pool.name()),
                            registry::variant_key(op)
                        ));
                    }
                    // SIMD can execute a mixed host-resident pipeline. Keep
                    // scanning its descriptors so an operation after a
                    // CPU-only boundary can still select the SIMD planner.
                    // GPU remains an all-operations route and stops at the
                    // first unsupported descriptor.
                    if pool.name() != Backend::Simd {
                        break;
                    }
                }
            }
            if pool.name() == Backend::Simd && supports_any {
                return Ok((pool.name(), fallback_reason, true));
            }
            if supports_all {
                // The support scan above is the validation pass for automatic
                // routing. Carry that fact to preparation so the same
                // registry queries are not repeated immediately before the
                // backend executor runs.
                return Ok((pool.name(), fallback_reason, true));
            }
        }
    }
    Ok((
        Backend::Cpu,
        fallback_reason.or_else(|| {
            active_set
                .iter()
                .any(|backend| *backend != Backend::Cpu)
                .then(|| "no active non-CPU backend supports the complete pipeline".to_string())
        }),
        false,
    )) // universal fallback
}

fn backend_label(backend: Backend) -> &'static str {
    match backend {
        Backend::Cpu => "CPU",
        Backend::Gpu => "GPU",
        Backend::Simd => "SIMD",
    }
}

fn is_simd_capability_error(error: &PilError) -> bool {
    matches!(error, PilError::NotImplementedError(message) if message.starts_with("SIMD "))
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedExecution {
    requested_backend: Option<Backend>,
    selected_backend: Backend,
    route_ns: u64,
    validation_ns: u64,
    fallback_reason: Option<String>,
}

/// Performs the route and validation phases before a source image is
/// materialized. Keeping this separate preserves the explicit-backend contract
/// while allowing the actual backend phase to be measured later.
pub(crate) fn prepare_execution(
    ops: &[PipelineOp],
    requested_backend: Option<Backend>,
) -> Result<PreparedExecution, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let (selected_backend, fallback_reason, support_checked) =
        route_decision(ops, requested_backend)?;
    let route_ns = elapsed_ns(route_start);

    let validation_start = timed.then(pipeline_timestamp).flatten();
    if !support_checked {
        validate_backend_support(selected_backend, ops)?;
    }
    let validation_ns = elapsed_ns(validation_start);

    Ok(PreparedExecution {
        requested_backend,
        selected_backend,
        route_ns,
        validation_ns,
        fallback_reason,
    })
}

/// Validates that a compiled backend has native support for every operation.
///
/// Image pipelines call this before materializing their source so an explicit
/// backend lock reports the first unsupported operation before any decode or
/// nested-pipeline error.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for an unavailable backend or the first
/// operation without a native implementation on the selected backend.
pub fn validate_backend_support(backend: Backend, ops: &[PipelineOp]) -> Result<(), PilError> {
    let pool = pools()
        .iter()
        .find(|pool| pool.name() == backend)
        .ok_or_else(|| PilError::ValueError(format!("Backend {:?} not available", backend)))?;
    for op in ops {
        if pool.supports(op)? {
            continue;
        }
        if matches!(backend, Backend::Simd | Backend::Gpu) {
            let key = registry::variant_key(op);
            record_pipeline_operation_unsupported(key);
            return Err(PilError::NotImplementedError(format!(
                "{} does not support {key}",
                backend_label(backend)
            )));
        }
        let name = match backend {
            Backend::Cpu => "CPU",
            Backend::Gpu => "GPU",
            Backend::Simd => "SIMD",
        };
        return Err(PilError::ValueError(format!(
            "{name}: no native impl for {}",
            registry::variant_key(op)
        )));
    }
    Ok(())
}

fn merge_pipeline_resource_telemetry(
    total: &mut Option<PipelineResourceTelemetry>,
    next: Option<PipelineResourceTelemetry>,
) {
    let Some(next) = next else {
        return;
    };
    let total = total.get_or_insert_with(PipelineResourceTelemetry::default);
    total.upload_bytes = total.upload_bytes.saturating_add(next.upload_bytes);
    total.readback_bytes = total.readback_bytes.saturating_add(next.readback_bytes);
    total.auxiliary_bytes = total.auxiliary_bytes.saturating_add(next.auxiliary_bytes);
    total.parameter_bytes = total.parameter_bytes.saturating_add(next.parameter_bytes);
    total.retained_cache_bytes = total.retained_cache_bytes.max(next.retained_cache_bytes);
    total.full_frame_copy_count = total
        .full_frame_copy_count
        .saturating_add(next.full_frame_copy_count);
    total.mode_conversion_count = total
        .mode_conversion_count
        .saturating_add(next.mode_conversion_count);
    total.host_buffer_count = total.host_buffer_count.saturating_add(next.host_buffer_count);
    total.host_buffer_bytes = total.host_buffer_bytes.saturating_add(next.host_buffer_bytes);
    total.peak_live_host_bytes = total.peak_live_host_bytes.max(next.peak_live_host_bytes);
    total.fused_operation_count = total
        .fused_operation_count
        .saturating_add(next.fused_operation_count);
    total.host_allocation_count = total
        .host_allocation_count
        .saturating_add(next.host_allocation_count);
    total.host_allocated_bytes = total
        .host_allocated_bytes
        .saturating_add(next.host_allocated_bytes);
}

/// Execute an automatic CPU/SIMD pipeline as contiguous host-resident
/// segments.
///
/// This planner is intentionally conservative about segment extension. A
/// segment may include only operations that preserve the current concrete
/// layout; crop, transpose, extraction, alpha promotion, conversion, and
/// other shape/mode-changing operations end the segment. The next operation
/// is then checked against the actual owned output, so no backend is entered
/// with stale capability information and no Python-level materialization is
/// needed at a CPU↔SIMD boundary.
fn execute_automatic_simd_segments(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<
    (
        DynamicImage,
        Backend,
        Option<String>,
        Option<PipelineResourceTelemetry>,
    ),
    PilError,
> {
    let simd = pools()
        .iter()
        .find(|pool| pool.name() == Backend::Simd)
        .ok_or_else(|| PilError::ValueError("SIMD backend not available".into()))?;
    let cpu = pools()
        .iter()
        .find(|pool| pool.name() == Backend::Cpu)
        .ok_or_else(|| PilError::ValueError("CPU backend not available".into()))?;

    let mut current: Option<DynamicImage> = None;
    let mut previous_backend = None;
    let mut used_simd = false;
    let mut fallback_reason = None;
    let mut resources = None;
    let mut index = 0usize;
    let mut current_mode = pool_simd::ops::adapters::simd_initial_mode(img, ops, mode);

    while index < ops.len() {
        let input = current.as_ref().unwrap_or(img);
        let op_mode = current_mode.as_deref();
        let backend = if simd.supports_for_image(&ops[index], input, op_mode)? {
            used_simd = true;
            Backend::Simd
        } else {
            fallback_reason.get_or_insert_with(|| {
                format!(
                    "SIMD does not support {} for the current image layout/mode",
                    registry::variant_key(&ops[index])
                )
            });
            Backend::Cpu
        };

        let mut end = index + 1;
        if backend == Backend::Simd {
            // Every operation in this segment is checked against the same
            // concrete layout. The last operation may change that layout; it
            // is included, then the segment ends before the next check.
            while end < ops.len()
                && adapters_preserve_native_contract(&ops[end - 1])
                && simd.supports_for_image(&ops[end], input, op_mode)?
            {
                end += 1;
            }
        } else {
            // CPU can absorb adjacent CPU-only work, including draw batches,
            // as long as the current layout contract remains unchanged. Stop
            // before an operation that SIMD can execute so the next segment
            // can return to the vector path without replaying this segment.
            while end < ops.len()
                && adapters_preserve_native_contract(&ops[end - 1])
                && !simd.supports_for_image(&ops[end], input, op_mode)?
            {
                if !cpu.supports(&ops[end])? {
                    break;
                }
                end += 1;
            }
        }

        if previous_backend.is_some_and(|previous| previous != backend) {
            record_pipeline_operation_handoff(1);
        }
        let pool = if backend == Backend::Simd { simd } else { cpu };
        let segment_ops = &ops[index..end];
        let next = pool.execute_batch(segment_ops, input, op_mode)?;
        merge_pipeline_resource_telemetry(&mut resources, take_pipeline_resource_telemetry());
        current = Some(next);
        previous_backend = Some(backend);
        for op in segment_ops {
            current_mode = pool_simd::ops::adapters::simd_mode_after_op(op, current_mode.as_deref());
        }
        index = end;
    }

    let result = current.unwrap_or_else(|| img.clone());
    if let Some(resource) = resources {
        record_pipeline_resource_telemetry(resource);
    }
    let actual_backend = if used_simd {
        Backend::Simd
    } else {
        Backend::Cpu
    };
    Ok((result, actual_backend, fallback_reason, resources))
}

fn adapters_preserve_native_contract(op: &PipelineOp) -> bool {
    pool_simd::ops::adapters::preserves_native_contract(op)
}

/// Executes a previously routed and validated pipeline, recording the bounded
/// backend phase when telemetry is enabled.
pub(crate) fn execute_prepared(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    if prepared.requested_backend.is_none() && prepared.selected_backend == Backend::Simd {
        let (result, actual_backend, mixed_reason, mut resource) =
            execute_automatic_simd_segments(ops, img, mode)?;
        if timed {
            let allocation = take_pipeline_allocation_telemetry();
            if let Some(resource) = resource.as_mut() {
                resource.host_allocation_count = allocation.allocation_count;
                resource.host_allocated_bytes = allocation.allocated_bytes;
            }
            // The segment executor publishes the aggregate so the ordinary
            // receipt boundary remains compatible with single-backend runs.
            let _ = take_pipeline_resource_telemetry();
            let _ = take_pipeline_backend_override();
            let _ = take_pipeline_dispatch_count();
            let (resize_coeff_cache_hits, resize_coeff_cache_misses) =
                take_pipeline_resize_coeff_cache_stats();
            record_pipeline_telemetry(PipelineTelemetry {
                requested_backend: prepared.requested_backend,
                actual_backend,
                operation_count: ops.len(),
                route_ns: prepared.route_ns,
                validation_ns: prepared.validation_ns,
                backend_ns: elapsed_ns(backend_start),
                dispatch_count: None,
                fallback_reason: mixed_reason.or_else(|| prepared.fallback_reason.clone()),
                resource,
                resize_coeff_cache_hits,
                resize_coeff_cache_misses,
            });
        }
        return Ok(result);
    }
    // Registry support is intentionally checked before materialization so a
    // malformed explicit request fails early.  Contextual SIMD support needs
    // the concrete image, however: the same operation can be native for one
    // layout and CPU-only for another.  Automatic routing may recover by
    // selecting CPU here; an explicit SIMD lock is a strict capability audit
    // and must report the unsupported contract instead of entering an adapter
    // that would silently delegate to CPU.
    let mut effective_backend = prepared.selected_backend;
    let mut contextual_fallback_reason = prepared.fallback_reason.clone();
    if prepared.selected_backend == Backend::Simd {
        if prepared.requested_backend == Some(Backend::Simd) {
            if let Some(op) = pool_simd::ops::adapters::first_unsupported_simd_op(img, ops, mode) {
                let key = registry::variant_key(op);
                record_pipeline_operation_unsupported(key);
                return Err(PilError::NotImplementedError(format!(
                    "SIMD does not support {key} for the current image layout/mode"
                )));
            }
        }
        // Automatic routing is planned operation-by-operation by
        // `execute_automatic_simd_segments`. Do not reject the whole pipeline
        // using the final output mode: an earlier operation may still be
        // native, and the planner tracks each intermediate logical mode.
    }
    for pool in pools() {
        if pool.name() == effective_backend {
            let estimated_dispatch_count = timed.then(|| pool.dispatch_count(ops)).flatten();
            let execution = match if prepared.requested_backend == Some(Backend::Gpu) {
                pool.execute_batch_strict(ops, img, mode)
            } else {
                pool.execute_batch(ops, img, mode)
            } {
                Err(error)
                    if effective_backend == Backend::Simd
                        && prepared.requested_backend != Some(Backend::Simd)
                        && is_simd_capability_error(&error) =>
                {
                    // A later operation can change the concrete layout after
                    // the initial contextual scan. SIMD must not call CPU
                    // from inside an adapter; retry the original host image
                    // through the CPU pool instead. Clear partial SIMD
                    // receipts first so the final sample describes the
                    // operation that produced the returned pixels.
                    effective_backend = Backend::Cpu;
                    contextual_fallback_reason.get_or_insert(error.to_string());
                    if timed {
                        reset_pipeline_allocation_telemetry();
                        reset_pipeline_operation_telemetry();
                        let _ = take_pipeline_resource_telemetry();
                        let _ = take_pipeline_backend_override();
                        let _ = take_pipeline_dispatch_count();
                        let _ = take_pipeline_resize_coeff_cache_stats();
                    }
                    record_pipeline_backend_fallback(error.to_string());
                    let cpu = pools()
                        .iter()
                        .find(|candidate| candidate.name() == Backend::Cpu)
                        .ok_or_else(|| PilError::ValueError("CPU backend not available".into()))?;
                    cpu.execute_batch(ops, img, mode)
                }
                result => result,
            };
            return match execution {
                Ok(result) => {
                    if timed {
                        let allocation = take_pipeline_allocation_telemetry();
                        let resource = take_pipeline_resource_telemetry().map(|mut resource| {
                            resource.host_allocation_count = allocation.allocation_count;
                            resource.host_allocated_bytes = allocation.allocated_bytes;
                            resource
                        });
                        let backend_override = take_pipeline_backend_override();
                        let observed_dispatch_count = take_pipeline_dispatch_count();
                        let actual_backend = backend_override
                            .as_ref()
                            .map(|(backend, _)| *backend)
                            // Contextual support is checked after the source
                            // image is available.  Automatic SIMD routing can
                            // therefore downgrade to CPU without an adapter
                            // override being recorded.  Report the executor
                            // that actually produced the pixels, not the
                            // operation-only route selected before
                            // materialization.
                            .unwrap_or(effective_backend);
                        let fallback_reason = backend_override
                            .as_ref()
                            .map(|(_, reason)| reason.clone())
                            .or_else(|| contextual_fallback_reason.clone());
                        let (resize_coeff_cache_hits, resize_coeff_cache_misses) =
                            take_pipeline_resize_coeff_cache_stats();
                        record_pipeline_telemetry(PipelineTelemetry {
                            requested_backend: prepared.requested_backend,
                            actual_backend,
                            operation_count: ops.len(),
                            route_ns: prepared.route_ns,
                            validation_ns: prepared.validation_ns,
                            backend_ns: elapsed_ns(backend_start),
                            dispatch_count: (actual_backend == effective_backend)
                                .then_some(observed_dispatch_count.or(estimated_dispatch_count))
                                .flatten(),
                            fallback_reason,
                            resource,
                            resize_coeff_cache_hits,
                            resize_coeff_cache_misses,
                        });
                    }
                    Ok(result)
                }
                Err(error) => {
                    if timed {
                        let _ = take_pipeline_resource_telemetry();
                        let _ = take_pipeline_backend_override();
                        let _ = take_pipeline_dispatch_count();
                        let _ = take_pipeline_resize_coeff_cache_stats();
                    }
                    Err(error)
                }
            };
        }
    }
    Err(PilError::ValueError(format!(
        "Backend {:?} not available",
        prepared.selected_backend
    )))
}
