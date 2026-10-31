pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod wasm_simd128;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
#[cfg(target_arch = "x86_64")]
pub(crate) mod ssse3;
