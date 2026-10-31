//! Pillow's scalar float mode strategy for Resize.

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx;
pub(crate) mod default;
pub(crate) mod ops;
