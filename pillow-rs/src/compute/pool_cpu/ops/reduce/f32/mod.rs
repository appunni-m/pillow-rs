#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx;
pub(crate) mod default;
pub(crate) mod helpers;
pub(crate) mod kernels;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
pub(crate) mod ops;
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature = "sse2",
    not(target_feature = "avx")
))]
pub(crate) mod sse2;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod wasm_simd128;
