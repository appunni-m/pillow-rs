//! Wasm SIMD128 entry point for the shared portable Chops vector kernel.
pub(super) use super::super::helpers::vector::execute;
pub(super) use super::super::helpers::fusion::execute_vector as execute_fused;
