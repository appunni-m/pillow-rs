//! Shared exact alpha-composite arithmetic.

pub(crate) mod alpha_composite;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod alpha_composite_avx512f;
pub(crate) mod alpha_composite_vector;
pub(crate) mod blend;
pub(crate) mod composite;
pub(crate) mod composite_module_support;
pub(crate) mod merge;
pub(crate) mod native_bytes;
pub(crate) mod preparation;
pub(crate) mod strategy;

pub(crate) use composite_module_support::{
    composite_wrapper_mask_alpha, declare_composite_module_support,
    validate_composite_wrapper_dimensions,
};
