//! CMYK8 source-mode conversion strategies.

pub(crate) mod default;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2"),
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(crate) mod helpers;
pub(crate) mod ops;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
#[cfg(target_arch = "x86_64")]
pub(crate) mod ssse3;
#[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
pub(crate) mod sse2;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod wasm_simd128;
