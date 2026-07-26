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
//! key, and optional GPU/SIMD support together. The registry module is the source
//! of truth used by [`crate::compute::route`] and
//! [`crate::compute::execute_batch`].

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image_slash_star::DynamicImage;
use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard};

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
}

// ── BackendImpl trait ──────────────────────────────────────────────────────

/// Implementation contract for a compute backend.
///
/// Backends advertise operation support before execution so the router can keep
/// a pipeline on one backend. Implementations should return errors instead of
/// panicking when runtime resources are unavailable.
pub trait BackendImpl: Send + Sync {
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
}

// ── Modules ────────────────────────────────────────────────────────────────

// AS PER DESIGN — DO NOT REMOVE:
// backend_op: BackendOp trait for per-backend capability detection
// op_def:     define_op! macro — single-definition op registration
/// Per-backend capability trait used by operation descriptors.
pub mod backend_op;
/// Macro-backed operation registration helpers.
pub mod op_def;

mod pool_cpu;
#[cfg(feature = "gpu")]
mod pool_gpu;
mod pool_simd;
pub mod registry;

pub use pool_cpu::CpuPool;
#[cfg(feature = "gpu")]
pub use pool_gpu::GpuPool;
pub use pool_simd::SimdPool;

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

/// Picks the best active backend that supports every operation in `ops`.
///
/// Passing `explicit` bypasses automatic routing and returns that backend even
/// if it is inactive or cannot support the batch; execution will report the
/// actual backend error later. Without an explicit backend, routing prefers
/// active backends by priority and falls back to [`Backend::Cpu`].
pub fn route(ops: &[PipelineOp], explicit: Option<Backend>) -> Result<Backend, PilError> {
    if let Some(b) = explicit {
        return Ok(b);
    }
    let active_set = active_lock()?;
    for pool in pools() {
        if active_set.contains(&pool.name()) {
            let mut supports_all = true;
            for op in ops {
                if !pool.supports(op)? {
                    supports_all = false;
                    break;
                }
            }
            if supports_all {
                return Ok(pool.name());
            }
        }
    }
    Ok(Backend::Cpu) // universal fallback
}

/// Validates that a compiled backend has native support for every operation.
///
/// Image pipelines call this before materializing their source so an explicit
/// backend lock reports the first unsupported operation before any decode or
/// nested-pipeline error. [`execute_batch`] repeats the validation for callers
/// that already own a materialized image.
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

/// Executes `ops` on a specific backend.
///
/// `mode` carries Pillow mode tags such as `"P"` or `"F"` when the
/// [`DynamicImage`] color type is not enough to describe semantics.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `backend` is not compiled into this
/// crate, or the backend returns an operation-specific error.
pub fn execute_batch(
    backend: Backend,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    validate_backend_support(backend, ops)?;
    for pool in pools() {
        if pool.name() == backend {
            return pool.execute_batch(ops, img, mode);
        }
    }
    Err(PilError::ValueError(format!(
        "Backend {:?} not available",
        backend
    )))
}
