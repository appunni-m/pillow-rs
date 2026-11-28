//! Geometry calculations shared across compute backends.

mod affine;

#[cfg(feature = "gpu")]
pub(crate) use affine::affine_nearest_fixed;
