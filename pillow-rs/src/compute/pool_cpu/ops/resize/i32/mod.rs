//! Pillow's signed 32-bit scalar sample strategy for Resize.

#[cfg(target_arch = "x86_64")]
pub(crate) mod avx;
pub(crate) mod default;
pub(crate) mod helpers;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
pub(crate) mod ops;
