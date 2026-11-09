//! Shared transform sampling math used by exact mode strategies.

pub(crate) mod affine;
pub(crate) mod alpha;
pub(crate) mod architecture_support;
pub(crate) mod byte;
pub(crate) mod byte_mode;
pub(crate) mod byte_nearest;
pub(crate) mod hidden_lanes;
pub(crate) mod luma16;
pub(crate) mod projective;
pub(crate) mod scalar;
pub(super) mod strategy;
pub(crate) mod vector_affine_bilinear;
pub(crate) mod vector_affine_nearest;
pub(crate) mod vector_luma16;
pub(crate) mod vector_projective_nearest;
pub(crate) mod vector_samples;

pub(crate) use byte_mode::{
    byte_mode_default, byte_mode_mesh_record, byte_mode_nearest, byte_mode_vector,
    luma16_mode_default_into, luma16_mode_vector,
};
