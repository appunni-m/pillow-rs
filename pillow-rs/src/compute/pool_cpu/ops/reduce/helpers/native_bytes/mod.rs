//! Native interleaved byte reducers shared by exact mode strategies.

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx2;
mod general;
mod rgb;
mod rgb_kernels;
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2",
        not(target_feature = "avx2")
    ),
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(crate) mod wide;

pub(crate) use general::{reduce_bytes, reduce_lab_bytes};
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        any(target_feature = "sse2", target_feature = "avx2")
    ),
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128")
))]
pub(crate) use rgb::reduce_sparse_2x2_if_applicable;
pub(crate) use rgb::{reduce_rgb, reduce_rgb_with_sparse_rows};
