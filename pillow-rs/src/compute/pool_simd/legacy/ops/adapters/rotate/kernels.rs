//! Rotate-family native kernel entry points.

mod native;
#[cfg(not(feature = "parallel"))]
mod nearest;

pub(super) use self::native::{
    native_identity_rotate, simd_bilinear_rotate_native, simd_empty_rotate_native,
    simd_nearest_rotate_native, simd_right_angle_rotate_native, simd_typed_bilinear_rotate_native,
};
#[cfg(not(feature = "parallel"))]
pub(crate) use self::nearest::copy_contiguous_rgb_runs;
