//! Exact ImageRgb8 enhancement strategies.
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod avx;
pub(crate) mod neon;
pub(crate) mod sse2;
pub(crate) mod wasm_simd128;
pub(crate) mod avx2;
