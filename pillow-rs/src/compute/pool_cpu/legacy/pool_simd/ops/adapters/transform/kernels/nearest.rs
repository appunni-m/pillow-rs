//! Transform nearest kernel family, split by coordinate map.

mod affine;
mod projective;

pub(in super::super) use self::affine::simd_affine_nearest_transform_bytes;
pub(in super::super) use self::projective::simd_projective_nearest_transform_bytes;
