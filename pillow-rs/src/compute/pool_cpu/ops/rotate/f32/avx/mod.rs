//! AVX/FMA mode adapter reusing the shared vector strategy.

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) use super::avx2::rotate_impl;
