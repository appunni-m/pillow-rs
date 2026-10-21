//! Helpers for resolving an unambiguous active backend.

use super::active_lock;
use super::{Backend, active_backends};
use crate::error::PilError;

/// Whether an unlocked pipeline can only be routed to the serial CPU backend.
/// A CPU-locked pipeline is handled directly by its caller; this check keeps
/// narrow CPU-only fusion admission from intercepting normal multi-backend
/// routing.
pub(crate) fn cpu_is_only_active_backend() -> bool {
    matches!(active_backends(), Ok(active) if active.as_slice() == [Backend::Cpu])
}

/// Whether an unlocked pipeline is exclusively routed through SIMD.
pub(crate) fn simd_is_only_active_backend() -> bool {
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
