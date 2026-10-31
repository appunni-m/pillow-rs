//! Shared transform sampling math used by exact mode strategies.

pub(crate) mod affine;
pub(crate) mod alpha;
pub(crate) mod byte;
pub(crate) mod byte_nearest;
pub(crate) mod architecture_support;
mod byte_mode;
pub(crate) mod projective;
pub(crate) mod scalar;
pub(crate) mod luma16;
pub(crate) mod vector_affine_nearest;
pub(crate) mod vector_affine_bilinear;
pub(crate) mod vector_projective_nearest;
pub(crate) mod vector_luma16;
pub(crate) mod vector_samples;
pub(super) mod strategy;
