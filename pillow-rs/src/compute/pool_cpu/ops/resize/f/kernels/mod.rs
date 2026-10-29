//! F-mode kernel implementations selected by the F strategy.

pub(super) mod default;
#[cfg(target_arch = "x86_64")]
pub(super) mod fma;
