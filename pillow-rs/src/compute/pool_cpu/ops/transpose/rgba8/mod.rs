use crate::raster::{RgbaImage};

pub(crate) type Input = RgbaImage;
pub(crate) mod default;
pub(crate) mod ops;
#[cfg(target_arch = "x86_64")]
pub(crate) mod avx2;

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
#[path = "sse2/transpose_impl.rs"]
pub(crate) mod sse2;

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[path = "neon/transpose_impl.rs"]
pub(crate) mod neon;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[path = "wasm_simd128/transpose_impl.rs"]
pub(crate) mod wasm_simd128;
