//! Shared exact alpha-composite arithmetic.

pub(crate) mod alpha_composite;
pub(crate) mod alpha_composite_vector;
pub(crate) mod blend;
pub(crate) mod composite;
pub(crate) mod composite_module_support;
pub(crate) mod merge;
pub(crate) mod native_bytes;
pub(crate) mod strategy;

pub(crate) use composite_module_support::{
    composite_wrapper_mask_alpha, declare_composite_module_support,
    validate_composite_wrapper_dimensions,
};
