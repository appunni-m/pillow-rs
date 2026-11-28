//! Exact ImageF32 Resize strategies and their native helpers.

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f;
pub(crate) mod default;
pub(crate) mod helpers;
#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;
pub(crate) mod ops;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod sse2;
#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm_simd128;
#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;
