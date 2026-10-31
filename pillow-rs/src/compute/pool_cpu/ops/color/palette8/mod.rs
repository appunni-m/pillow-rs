//! Exact Pillow P source-mode strategy for palette remapping.

pub(crate) mod default;
pub(crate) mod neon;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod wasm_simd128;
