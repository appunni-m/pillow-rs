//! Native ImageI32 Thumbnail strategies.

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx;
pub(crate) mod default;
pub(crate) mod helpers;
pub(crate) mod ops;
