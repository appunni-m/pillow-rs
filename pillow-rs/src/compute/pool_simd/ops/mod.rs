//! SIMD-accelerated operation implementations.
//!
//! The portable scalar implementation is the reference and fallback path.
//!
//! `scalar` is always compiled as the reference + fallback.

pub(crate) mod adapters;
mod scalar; // always available — reference implementation // SIMD → registry adapter wrappers
