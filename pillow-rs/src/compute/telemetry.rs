//! Bounded execution telemetry and receipt collection for compute backends.

use super::{Backend, PipelineTelemetrySample};

#[path = "telemetry/eager_cpu.rs"]
mod eager_cpu;
pub(super) use eager_cpu::pipeline_timestamp;
pub(crate) use eager_cpu::{
    begin_eager_cpu_operation_telemetry, cancel_eager_cpu_operation_telemetry,
    finish_eager_cpu_operation_telemetry, record_eager_cpu_operation_receipt,
};

use crate::error::PilError;
use crate::raster::DynamicImage;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
#[cfg(not(test))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(test))]
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[path = "telemetry/origins.rs"]
mod origins;
pub use origins::PipelineTelemetryOrigin;
pub(crate) use origins::{
    RestorePipelineOperationOrigins, capture_active_origin, enter_operation_origins,
    origin_for_event, replace_active_origin,
};

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
    /// Result bytes made host-visible for readback, through a staging copy or
    /// a directly mapped primary buffer. This is readback volume, not a count
    /// of physical device copies; those are recorded by full_frame_copy_count.
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
#[derive(Debug, Clone)]
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
    /// Public workflow call that created this operation, when captured.
    #[doc(hidden)]
    pub source: Option<PipelineTelemetryOrigin>,
}

#[derive(Debug, Clone)]
struct PipelineOperationSample {
    operation: &'static str,
    path: &'static str,
    vector_block_count: u64,
    scalar_tail_count: u64,
    mode_conversion_count: u64,
    handoff_count: u64,
    source: Option<PipelineTelemetryOrigin>,
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
pub(super) struct PipelineTelemetry {
    pub(super) requested_backend: Option<Backend>,
    pub(super) actual_backend: Backend,
    pub(super) operation_count: usize,
    pub(super) route_ns: u64,
    pub(super) validation_ns: u64,
    pub(super) backend_ns: u64,
    pub(super) dispatch_count: Option<u64>,
    pub(super) fallback_reason: Option<String>,
    pub(super) resource: Option<PipelineResourceTelemetry>,
    pub(super) resize_coeff_cache_hits: u64,
    pub(super) resize_coeff_cache_misses: u64,
}

#[cfg(not(test))]
static GPU_SHADER_COVERAGE_ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(not(test))]
static GPU_SHADER_COVERAGE: OnceLock<
    Mutex<BTreeMap<(&'static str, &'static str), GpuShaderDispatchCounters>>,
> = OnceLock::new();

thread_local! {
    #[cfg(test)]
    static GPU_SHADER_COVERAGE_ENABLED: Cell<bool> = const { Cell::new(false) };
    #[cfg(test)]
    static GPU_SHADER_COVERAGE: RefCell<BTreeMap<(&'static str, &'static str), GpuShaderDispatchCounters>> = const { RefCell::new(BTreeMap::new()) };
    static PIPELINE_TELEMETRY_ENABLED: Cell<bool> = const { Cell::new(false) };
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

pub(super) fn set_pipeline_telemetry_enabled(enabled: bool) -> bool {
    let previous = PIPELINE_TELEMETRY_ENABLED.with(|setting| setting.replace(enabled));
    if !enabled {
        LAST_PIPELINE_TELEMETRY.with(|last| {
            *last.borrow_mut() = None;
        });
        super::phase_telemetry::clear();
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

#[cfg(not(test))]
pub(super) fn set_gpu_shader_coverage_enabled(enabled: bool) -> bool {
    let previous = GPU_SHADER_COVERAGE_ENABLED.swap(enabled, Ordering::Relaxed);
    if !enabled
        && let Some(coverage) = GPU_SHADER_COVERAGE.get()
        && let Ok(mut coverage) = coverage.lock()
    {
        coverage.clear();
    }
    previous
}

#[cfg(test)]
pub(super) fn set_gpu_shader_coverage_enabled(enabled: bool) -> bool {
    let previous = GPU_SHADER_COVERAGE_ENABLED.with(|setting| setting.replace(enabled));
    if !enabled {
        GPU_SHADER_COVERAGE.with(|coverage| coverage.borrow_mut().clear());
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
    #[cfg(test)]
    if !GPU_SHADER_COVERAGE_ENABLED.with(Cell::get) {
        return;
    }
    #[cfg(not(test))]
    if !GPU_SHADER_COVERAGE_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    #[cfg(test)]
    GPU_SHADER_COVERAGE.with(|coverage| {
        let mut coverage = coverage.borrow_mut();
        let counters = coverage.entry((variant_name, shader_file)).or_default();
        counters.dispatches = counters.dispatches.saturating_add(1);
        counters.workgroups = counters.workgroups.saturating_add(workgroups);
    });
    #[cfg(not(test))]
    {
        let coverage = GPU_SHADER_COVERAGE.get_or_init(|| Mutex::new(BTreeMap::new()));
        let Ok(mut coverage) = coverage.lock() else {
            return;
        };
        let counters = coverage.entry((variant_name, shader_file)).or_default();
        counters.dispatches = counters.dispatches.saturating_add(1);
        counters.workgroups = counters.workgroups.saturating_add(workgroups);
    }
}

pub(super) fn take_gpu_shader_coverage() -> Vec<GpuShaderDispatchTelemetry> {
    #[cfg(test)]
    let counters = GPU_SHADER_COVERAGE.with(|coverage| std::mem::take(&mut *coverage.borrow_mut()));
    #[cfg(not(test))]
    let counters = {
        let Some(coverage) = GPU_SHADER_COVERAGE.get() else {
            return Vec::new();
        };
        let Ok(mut coverage) = coverage.lock() else {
            return Vec::new();
        };
        std::mem::take(&mut *coverage)
    };
    counters
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

#[cfg(all(test, feature = "gpu"))]
mod gpu_shader_coverage_tests {
    use super::{
        record_gpu_shader_dispatch, set_gpu_shader_coverage_enabled, take_gpu_shader_coverage,
    };

    #[test]
    fn shader_dispatch_coverage_is_isolated_between_test_threads() {
        struct RestoreCoverage(bool);
        impl Drop for RestoreCoverage {
            fn drop(&mut self) {
                set_gpu_shader_coverage_enabled(self.0);
            }
        }

        let previous = set_gpu_shader_coverage_enabled(true);
        let _restore = RestoreCoverage(previous);
        let _ = take_gpu_shader_coverage();

        std::thread::spawn(|| {
            record_gpu_shader_dispatch("other_thread", "other.wgsl", 1);
        })
        .join()
        .expect("test thread should finish");
        record_gpu_shader_dispatch("current_thread", "current.wgsl", 2);

        let coverage = take_gpu_shader_coverage();
        assert_eq!(coverage.len(), 1);
        assert_eq!(coverage[0].variant_name, "current_thread");
        assert_eq!(coverage[0].shader_file, "current.wgsl");
        assert_eq!(coverage[0].dispatches, 1);
        assert_eq!(coverage[0].workgroups, 2);
    }
}

pub(crate) fn reset_pipeline_allocation_telemetry() {
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| {
        *last.borrow_mut() = PipelineAllocationTelemetry::default();
    });
}

pub(crate) fn take_pipeline_allocation_telemetry() -> PipelineAllocationTelemetry {
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
                source: sample.source.clone(),
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
            handoff_count: PENDING_PIPELINE_HANDOFFS
                .with(|pending| std::mem::take(&mut *pending.borrow_mut())),
            source: origin_for_event(operation),
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
            // A fused executor may add the generic native-copy label after
            // an adapter has identified its architecture-specific kernel.
            // Preserve the more precise AVX2 receipt in that case.
            if sample.path == "avx2-transpose" && path == "native-copy" {
                return;
            }
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
            source: origin_for_event(operation),
        });
    });
}

/// Records one checked host pixel-buffer allocation for the active managed
/// benchmark sample.
/// Execute a synchronous SIMD terminal read directly into its caller-owned
/// result buffer and publish an honest receipt for the actual SIMD kernel.
/// Internal kernel allocations are intentionally not inferred from the final
/// host buffer accounting.
pub(crate) fn execute_eager_simd_bytes(
    operation: &'static str,
    input: &DynamicImage,
    output: &mut [u8],
    execute: impl FnOnce(&mut [u8]) -> Result<(u64, u64, &'static str), PilError>,
) -> Result<(), PilError> {
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
    let (vector_blocks, scalar_tail, path) = execute(output)?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        begin_pipeline_operation_telemetry(operation);
        record_pipeline_operation_vector_blocks(vector_blocks);
        record_pipeline_operation_scalar_tail(scalar_tail);
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();

        let mut resource = host_resource_telemetry(input);
        let output_bytes = output.len() as u64;
        resource.host_buffer_count = resource
            .host_buffer_count
            .saturating_add(u64::from(output_bytes != 0));
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        record_pipeline_resource_telemetry(resource);
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(Backend::Simd),
            actual_backend: Backend::Simd,
            operation_count: 1,
            route_ns: 0,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason: None,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(())
}

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

pub(super) fn pipeline_telemetry_enabled() -> bool {
    PIPELINE_TELEMETRY_ENABLED.with(Cell::get)
}

pub(super) fn take_pipeline_telemetry() -> Option<PipelineTelemetrySample> {
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

pub(super) fn take_pipeline_resize_coeff_cache_stats() -> (u64, u64) {
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

pub(super) fn take_pipeline_dispatch_count() -> Option<u64> {
    LAST_PIPELINE_DISPATCH_COUNT.with(|last| last.borrow_mut().take())
}

pub(super) fn take_pipeline_resource_telemetry() -> Option<PipelineResourceTelemetry> {
    LAST_PIPELINE_RESOURCE_TELEMETRY.with(|last| last.borrow_mut().take())
}

/// Record a backend-internal fallback so the public receipt reports the
/// executor that actually produced pixels, not only the router's request.
pub(crate) fn record_pipeline_backend_fallback(reason: impl Into<String>) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    restore_pipeline_backend_override((Backend::Cpu, reason.into()));
    record_pipeline_operation_handoff(1);
}

pub(super) fn take_pipeline_backend_override() -> Option<(Backend, String)> {
    LAST_PIPELINE_BACKEND_OVERRIDE.with(|last| last.borrow_mut().take())
}

/// Preserve a terminal backend identity while a segmented executor unwinds.
///
/// A GPU pipeline may execute an exact host-controlled prefix and then copy
/// or transform that result in a final GPU segment. The prefix must not leave
/// its CPU override attached to the outer receipt, but its fallback reason is
/// still useful evidence. Segmentation uses this setter after it has observed
/// the suffix executor; unlike [`record_pipeline_backend_fallback`], it does
/// not count another backend handoff.
pub(crate) fn restore_pipeline_backend_override(override_value: (Backend, String)) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_BACKEND_OVERRIDE.with(|last| {
        *last.borrow_mut() = Some(override_value);
    });
}

pub(super) fn record_pipeline_telemetry(sample: PipelineTelemetry) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_TELEMETRY.with(|last| {
        *last.borrow_mut() = Some(sample);
    });
}

pub(super) fn elapsed_ns(start: Option<Instant>) -> u64 {
    start
        .map(|start| start.elapsed().as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}
