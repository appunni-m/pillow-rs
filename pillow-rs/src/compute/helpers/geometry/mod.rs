//! Geometry calculations shared across compute backends.

mod affine;

pub(crate) use affine::affine_nearest_fixed;
