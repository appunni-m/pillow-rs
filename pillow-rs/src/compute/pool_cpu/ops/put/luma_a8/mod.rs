pub(crate) mod default;
pub(crate) mod ops;
#[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "ssse3")))]
pub(crate) mod ssse3;
#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
pub(crate) mod sse2;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod wasm_simd128;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
