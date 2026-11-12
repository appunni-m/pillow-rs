//! Active backend policy and the compiled executor-pool registry.
//!
#[cfg(feature = "gpu")]
use super::GpuPool;
use super::{Backend, BackendImpl, CpuPool};
use crate::error::PilError;
use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, OnceLock};

// ── Backend activation ─────────────────────────────────────────────────────

static ACTIVE: std::sync::OnceLock<Mutex<HashSet<Backend>>> = std::sync::OnceLock::new();

fn active() -> &'static Mutex<HashSet<Backend>> {
    ACTIVE.get_or_init(|| {
        let mut s = HashSet::new();
        s.insert(Backend::Cpu);
        Mutex::new(s)
    })
}

pub(super) fn active_lock() -> Result<MutexGuard<'static, HashSet<Backend>>, PilError> {
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(super) fn pools() -> &'static [Box<dyn BackendImpl>] {
    static POOLS: OnceLock<Vec<Box<dyn BackendImpl>>> = OnceLock::new();
    POOLS.get_or_init(|| {
        let mut v: Vec<Box<dyn BackendImpl>> = vec![
            Box::new(CpuPool),
            #[cfg(feature = "gpu")]
            Box::new(GpuPool),
        ];
        v.sort_by_key(|b| std::cmp::Reverse(b.priority()));
        v
    })
}

/// Returns every backend compiled into this crate.
/// Returns compiled executor backends. The legacy SIMD request alias is not
/// listed because it selects a CPU architecture strategy rather than a pool.
pub fn available_backends() -> Vec<Backend> {
    pools().iter().map(|p| p.name()).collect()
}

/// Whether an unlocked pipeline can only be routed to the serial CPU backend.
/// A CPU-locked pipeline is handled directly by its caller; this check keeps
/// narrow CPU-only fusion admission from intercepting normal multi-backend
/// routing.
pub(crate) fn cpu_is_only_active_backend() -> bool {
    matches!(active_backends(), Ok(active) if active.as_slice() == [Backend::Cpu])
}

/// Whether an unlocked pipeline is exclusively routed through the legacy SIMD
/// request alias, which selects CPU with the detected host architecture.
pub(crate) fn cpu_architecture_alias_is_only_active_backend() -> bool {
    matches!(active_backends(), Ok(active) if active.as_slice() == [Backend::Simd])
}

/// Select the highest-priority active backend for one eager image operation
/// without allocating the sorted vector returned by [`active_backends`].
pub(crate) fn preferred_active_backend() -> Result<Option<(Backend, bool)>, PilError> {
    let active = active_lock()?;
    let selected = [Backend::Gpu, Backend::Simd, Backend::Cpu]
        .into_iter()
        .find(|backend| active.contains(backend));
    Ok(selected.map(|backend| (backend, active.len() == 1)))
}
