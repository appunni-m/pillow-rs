//! Compute backend dispatch — multi-backend image pipeline execution.
//!
//! ## Architecture
//! - `Backend` enum: Cpu, Gpu, Simd
//! - `BackendImpl` trait: one impl per backend — `supports()`, `execute_batch()`, bridge methods
//! - `registry.rs`: maps every PipelineOp → which backends implement it
//! - Router: picks best backend for pipeline, bridges data as needed
//!
//! ## Adding a new PipelineOp
//! 1. Add variant to `PipelineOp` enum
//! 2. Add CPU impl in `pool_cpu/ops/<category>.rs`
//! 3. Add GPU shader in `pool_gpu/shaders/<name>.wgsl` (optional)
//! 4. Register in `registry.rs` — one line
//!    Done. No other files touched.
//!
//! ## Principles
//! - CPU is universal fallback — registered for every op
//! - Pipeline picks ONE backend — no per-op switching within a pipeline
//! - Bridge unrestricted — can transfer between any two backends
//! - SIMD-ready — add `pool_simd/` and register ops

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image::DynamicImage;
use std::collections::HashSet;
use std::sync::Mutex;

// ── Backend ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Backend {
    Cpu = 0,
    Gpu = 1,
    Simd = 2,
}

impl Backend {
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

/// Every backend implements this trait. CPU is the universal fallback.
pub trait BackendImpl: Send + Sync {
    fn name(&self) -> Backend;
    fn priority(&self) -> u8; // higher = preferred
    fn supports(&self, op: &PipelineOp) -> bool;
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError>;
}

// ── Modules ────────────────────────────────────────────────────────────────

mod pool_cpu;
#[cfg(not(target_arch = "wasm32"))]
mod pool_gpu;
mod pool_simd;
pub mod registry;

pub use pool_cpu::CpuPool;
#[cfg(not(target_arch = "wasm32"))]
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

pub fn enable_backend(b: Backend) -> bool {
    active().lock().unwrap().insert(b)
}
pub fn disable_backend(b: Backend) -> bool {
    active().lock().unwrap().remove(&b)
}
pub fn backend_enabled(b: Backend) -> bool {
    active().lock().unwrap().contains(&b)
}

pub fn active_backends() -> Vec<Backend> {
    let a = active().lock().unwrap();
    let mut v: Vec<Backend> = a.iter().copied().collect();
    v.sort_by_key(|b| std::cmp::Reverse(*b as u8));
    v
}

// ── Pool registry ──────────────────────────────────────────────────────────

use std::sync::OnceLock;

fn pools() -> &'static [Box<dyn BackendImpl>] {
    static POOLS: OnceLock<Vec<Box<dyn BackendImpl>>> = OnceLock::new();
    POOLS.get_or_init(|| {
        let mut v: Vec<Box<dyn BackendImpl>> = vec![
            Box::new(CpuPool),
            #[cfg(not(target_arch = "wasm32"))]
            Box::new(GpuPool),
            Box::new(SimdPool),
        ];
        v.sort_by_key(|b| std::cmp::Reverse(b.priority()));
        v
    })
}

pub fn available_backends() -> Vec<Backend> {
    pools().iter().map(|p| p.name()).collect()
}

// ── Router ─────────────────────────────────────────────────────────────────

/// Pick best active backend that supports ALL ops. Falls back to CPU.
pub fn route(ops: &[PipelineOp], explicit: Option<Backend>) -> Backend {
    if let Some(b) = explicit {
        return b;
    }
    let active_set = active().lock().unwrap();
    for pool in pools() {
        if active_set.contains(&pool.name()) && ops.iter().all(|op| pool.supports(op)) {
            return pool.name();
        }
    }
    Backend::Cpu // universal fallback
}

/// Execute a batch on a specific backend.
pub fn execute_batch(
    backend: Backend,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
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
