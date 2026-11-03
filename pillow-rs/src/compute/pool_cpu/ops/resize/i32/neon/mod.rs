//! AArch64 NEON strategy for filtered signed-32 sample Resize.

pub(crate) mod kernels;
pub(crate) mod plan;
pub(crate) mod resize_impl;

pub(super) use plan::LANES;
