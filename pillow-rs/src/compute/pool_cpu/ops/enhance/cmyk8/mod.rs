//! Exact ImageCmyk8 enhancement strategies.
pub(crate) mod avx;
pub(crate) mod avx2;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx512f;
pub(crate) mod default;
pub(crate) mod neon;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod wasm_simd128;
