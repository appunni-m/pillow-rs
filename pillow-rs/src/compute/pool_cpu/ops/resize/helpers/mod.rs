//! Shared typed kernels for Resize modes with identical sample arithmetic.

pub(crate) mod typed_nearest;
pub(crate) mod coefficients;
pub(crate) mod strategy;
pub(crate) mod luma16;
pub(crate) mod luma16_vector;
pub(crate) mod typed_f32;
pub(crate) mod typed_u16;
pub(crate) mod byte_samples;
pub(crate) mod native_nearest;
pub(crate) mod native_byte_vector;
pub(crate) mod alpha_samples;
