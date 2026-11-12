//! Wasm SIMD128-compiled portable-vector palette remapping.

pub(crate) mod remap_palette_impl;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) mod convert_impl;
