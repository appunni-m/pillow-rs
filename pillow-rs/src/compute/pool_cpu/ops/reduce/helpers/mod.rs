//! Operation-local geometry and typed kernels for Reduce.

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f_2x2;
pub(crate) mod hidden_lanes;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod i32_avx512f;
pub(crate) mod native_bytes;
pub(crate) mod native_samples;
pub(crate) mod scalar32;
pub(crate) mod strategy;
