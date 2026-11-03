//! Wasm SIMD128 entry point for the shared portable Chops vector kernel.
pub(super) use super::super::helpers::vector::execute;
pub(super) use super::super::helpers::offset::execute_bytes as execute_offset;
pub(super) use super::super::helpers::vector::execute_in_place;
