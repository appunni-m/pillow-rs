//! Exact-mode Pad strategy.

pub(crate) mod default;
pub(crate) mod sse2;
pub(crate) mod neon;
pub(crate) mod wasm_simd128;
pub(crate) mod ops;
