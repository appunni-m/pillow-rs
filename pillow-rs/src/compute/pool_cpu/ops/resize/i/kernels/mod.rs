//! I-mode kernel implementations selected by the I strategy.

pub(super) mod default;
#[cfg(target_arch = "x86_64")]
pub(super) mod fma;
