//! Convert strategies for premultiplied luma-alpha input.

pub(crate) mod default;
pub(crate) mod helpers;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;
pub(crate) mod ops;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
))]
pub(crate) mod sse2;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod wasm_simd128;
