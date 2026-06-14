use crate::error::PilError;
use crate::pipeline::PipelineOp;
use image::DynamicImage;
use std::collections::HashSet;
use std::sync::Mutex;

// ── ComputeBackend trait ──────────────────────────────────────────────────────

pub trait ComputeBackend: Send + Sync {
    fn name(&self) -> Backend;
    fn supports(&self, op: &PipelineOp) -> bool;
    fn execute_batch(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        explicit_mode: Option<&str>,
        palette: Option<&[u8]>,
    ) -> Result<DynamicImage, PilError>;
    fn priority(&self) -> u8;
}

// ── Backend enum ──────────────────────────────────────────────────────────────

/// Fixed set of compute backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Backend {
    Cpu = 0,
    Gpu = 1,
    // Future: Simd = 2, Cuda = 3, Metal = 4
}

impl Backend {
    /// Parse from a string (for binding layers). Case-insensitive.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            _ => None,
        }
    }

    /// All possible variants.
    pub const ALL: &[Self] = &[Self::Cpu, Self::Gpu];
}

// ── Submodules ────────────────────────────────────────────────────────────────

mod cpu_backend;
mod gpu_backend;
pub mod op_map;
mod registry;

pub use cpu_backend::CpuBackend;
pub use gpu_backend::GpuBackend;

// ── Available backends (detection) ────────────────────────────────────────────

use std::sync::OnceLock;

/// All backends that exist on this machine (detected at init).
pub fn get_backends() -> &'static [Box<dyn ComputeBackend>] {
    static BACKENDS: OnceLock<Vec<Box<dyn ComputeBackend>>> = OnceLock::new();
    BACKENDS.get_or_init(|| {
        op_map::init();
        let mut v: Vec<Box<dyn ComputeBackend>> = Vec::new();
        if let Some(gpu) = GpuBackend::new(4096 * 4096) {
            v.push(Box::new(gpu));
        }
        v.push(Box::new(CpuBackend));
        v.sort_by_key(|b| std::cmp::Reverse(b.priority() as i32));
        v
    })
}

/// Which backends exist on this machine.
pub fn available_backends() -> Vec<Backend> {
    get_backends().iter().map(|b| b.name()).collect()
}

/// Check if a backend exists on this machine.
pub fn has_backend(b: Backend) -> bool {
    get_backends().iter().any(|be| be.name() == b)
}

// ── Active backends (global toggle) ───────────────────────────────────────────

/// Currently active backends. Default: all available.
static ACTIVE_BACKENDS: OnceLock<Mutex<HashSet<Backend>>> = OnceLock::new();

fn active() -> &'static Mutex<HashSet<Backend>> {
    ACTIVE_BACKENDS.get_or_init(|| {
        // Default: CPU only (GPU backend is experimental and auto-merged)
        let mut s = HashSet::new();
        s.insert(Backend::Cpu);
        Mutex::new(s)
    })
}

/// Activate a backend so it participates in dispatch.
/// Returns true if the backend exists on this machine.
pub fn enable_backend(b: Backend) -> bool {
    if has_backend(b) {
        active().lock().unwrap().insert(b);
        true
    } else {
        false
    }
}

/// Deactivate a backend.
/// Returns true if it was active and is now removed.
pub fn disable_backend(b: Backend) -> bool {
    active().lock().unwrap().remove(&b)
}

/// Check if a specific backend is active.
pub fn backend_enabled(b: Backend) -> bool {
    active().lock().unwrap().contains(&b)
}

/// Currently active backends, sorted by priority (highest first).
pub fn active_backends() -> Vec<Backend> {
    let a = active().lock().unwrap();
    get_backends()
        .iter()
        .filter(|b| a.contains(&b.name()))
        .map(|b| b.name())
        .collect()
}

/// Active backends as trait objects (for dispatch).
pub fn get_active_objects() -> Vec<&'static Box<dyn ComputeBackend>> {
    let a = active().lock().unwrap();
    get_backends()
        .iter()
        .filter(|b| a.contains(&b.name()))
        .collect()
}

/// Find the first active backend that supports ALL given ops.
/// Returns None if no single backend covers everything.
pub fn find_backend_for_ops(ops: &[PipelineOp]) -> Option<Backend> {
    for backend_obj in get_active_objects() {
        if ops.iter().all(|op| backend_obj.supports(op)) {
            return Some(backend_obj.name());
        }
    }
    None
}

/// Execute a batch of operations on a specific backend.
pub fn execute_on(
    backend: Backend,
    ops: &[PipelineOp],
    img: &DynamicImage,
    explicit_mode: Option<&str>,
    palette: Option<&[u8]>,
) -> Result<DynamicImage, PilError> {
    for backend_obj in get_active_objects() {
        if backend_obj.name() == backend {
            return backend_obj.execute_batch(ops, img, explicit_mode, palette);
        }
    }
    Err(PilError::ValueError(format!(
        "Backend {:?} not available or not active",
        backend
    )))
}
