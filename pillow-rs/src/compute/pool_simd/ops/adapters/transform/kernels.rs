//! Transform sampling kernel family and shared vector helpers.

use super::super::rotate::rotate_fill_sample;
use super::super::{
    SIMD_F64_LANES, SIMD_RESIZE_NEAREST_BYTES, SimdRotateSample, bilinear_rotate_scalar,
    bilinear_rotate_vector, gather_rotate_neighborhood_vectors, rotate_fused_mul_add,
    rotate_unpremultiply_vector, simd_rotate_sample_coordinates, simd_unsupported,
};
use super::{
    native_affine_bilinear_layout, native_affine_luma16_transform_supported_for_image,
    native_affine_nearest_transform_channels,
};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, ImageBuffer, Luma};
use wide::{f64x8, i64x8, u8x16, u16x8};

mod bilinear;
mod luma16;
mod mesh;
mod nearest;

pub(super) use self::bilinear::{
    simd_affine_bilinear_transform_bytes, simd_projective_bilinear_transform_bytes,
};
pub(super) use self::luma16::simd_affine_luma16_transform_bytes;
pub(super) use self::mesh::simd_mesh_transform_bytes;
pub(super) use self::nearest::{
    simd_affine_nearest_transform_bytes, simd_projective_nearest_transform_bytes,
};
