//! Compute backend routing for deferred image pipelines.
//!
//! This module decides where a materialized [`crate::pipeline::PipelineOp`]
//! batch executes. The public surface is intentionally small: callers can
//! inspect compiled backends, enable or disable automatic routing choices, lock
//! an image pipeline to one backend, and execute a selected backend.
//!
//! # Routing Contract
//!
//! A pipeline executes on one backend for the whole batch. CPU is always present
//! and acts as the fallback. GPU and SIMD are selected only when they are active
//! and every operation in the batch reports support.
//!
//! # Adding Operations
//!
//! New operations must define the pipeline variant, CPU implementation, registry
//! key, and optional GPU/SIMD support together. The registry module is the
//! source of truth used by the routing and execution phases.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
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
    /// Number of full-frame copy boundaries observed by the backend.
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
    if before.dimensions() == after.dimensions() && before_bytes == after_bytes {
        telemetry.full_frame_copy_count = telemetry.full_frame_copy_count.saturating_add(1);
    }
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

thread_local! {
    static LAST_PIPELINE_TELEMETRY: RefCell<Option<PipelineTelemetry>> = const { RefCell::new(None) };
    static LAST_PIPELINE_RESOURCE_TELEMETRY: RefCell<Option<PipelineResourceTelemetry>> = const { RefCell::new(None) };
    static LAST_PIPELINE_BACKEND_OVERRIDE: RefCell<Option<(Backend, String)>> = const { RefCell::new(None) };
    static LAST_PIPELINE_DISPATCH_COUNT: RefCell<Option<u64>> = const { RefCell::new(None) };
    static LAST_PIPELINE_RESIZE_CACHE_STATS: RefCell<(u64, u64)> = const { RefCell::new((0, 0)) };
    static LAST_PIPELINE_ALLOCATION_TELEMETRY: RefCell<PipelineAllocationTelemetry> = const { RefCell::new(PipelineAllocationTelemetry { allocation_count: 0, allocated_bytes: 0 }) };
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
        reset_pipeline_allocation_telemetry();
    }
    previous
}

fn reset_pipeline_allocation_telemetry() {
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| {
        *last.borrow_mut() = PipelineAllocationTelemetry::default();
    });
}

fn take_pipeline_allocation_telemetry() -> PipelineAllocationTelemetry {
    LAST_PIPELINE_ALLOCATION_TELEMETRY.with(|last| std::mem::take(&mut *last.borrow_mut()))
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
#[allow(dead_code)]
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

use std::sync::OnceLock;

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
    // Plain Crop is bandwidth-only and the SIMD adapter uses the same native
    // row movement as CPU after avoiding packed-RGBA conversion. Without this
    // guard, automatic priority routing pays an extra adapter boundary for no
    // kernel benefit on a copy-only Crop batch. CropBorder remains eligible
    // for SIMD because the measured large native-border path is faster there.
    // Explicit SIMD requests still go through the real adapter so its parity
    // contract remains testable; this is only an automatic-routing policy.
    let avoid_simd_copy_adapter =
        !ops.is_empty() && ops.iter().all(|op| matches!(op, PipelineOp::Crop { .. }));
    for pool in pools() {
        if active_set.contains(&pool.name()) {
            if avoid_simd_copy_adapter && pool.name() == Backend::Simd {
                if fallback_reason.is_none() {
                    fallback_reason = Some("SIMD Crop delegates to native CPU row movement".into());
                }
                continue;
            }
            let mut supports_all = true;
            for op in ops {
                if !pool.supports(op)? {
                    supports_all = false;
                    if pool.name() != Backend::Cpu && fallback_reason.is_none() {
                        fallback_reason = Some(format!(
                            "{} does not support {}",
                            backend_label(pool.name()),
                            registry::variant_key(op)
                        ));
                    }
                    break;
                }
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
    let route_start = timed.then(Instant::now);
    let (selected_backend, fallback_reason, support_checked) =
        route_decision(ops, requested_backend)?;
    let route_ns = elapsed_ns(route_start);

    let validation_start = timed.then(Instant::now);
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
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(Instant::now);
    for pool in pools() {
        if pool.name() == prepared.selected_backend {
            let estimated_dispatch_count = timed.then(|| pool.dispatch_count(ops)).flatten();
            return match pool.execute_batch(ops, img, mode) {
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
                            .unwrap_or(prepared.selected_backend);
                        let fallback_reason = backend_override
                            .as_ref()
                            .map(|(_, reason)| reason.clone())
                            .or_else(|| prepared.fallback_reason.clone());
                        let (resize_coeff_cache_hits, resize_coeff_cache_misses) =
                            take_pipeline_resize_coeff_cache_stats();
                        record_pipeline_telemetry(PipelineTelemetry {
                            requested_backend: prepared.requested_backend,
                            actual_backend,
                            operation_count: ops.len(),
                            route_ns: prepared.route_ns,
                            validation_ns: prepared.validation_ns,
                            backend_ns: elapsed_ns(backend_start),
                            dispatch_count: (actual_backend == prepared.selected_backend)
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
