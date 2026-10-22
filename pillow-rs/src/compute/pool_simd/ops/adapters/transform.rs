//! SIMD Transform adapter admission and method selection.

use super::{
    native_affine_luma16_transform_supported_for_image,
    native_affine_nearest_transform_supported_for_image, native_mesh_transform_supported_for_image,
    native_projective_nearest_transform_supported_for_image, simd_affine_bilinear_transform_bytes,
    simd_affine_luma16_transform_bytes, simd_affine_nearest_transform_bytes,
    simd_mesh_transform_bytes, simd_projective_bilinear_transform_bytes,
    simd_projective_nearest_transform_bytes, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

/// Execute the native byte affine subset of `Image.transform`.
/// Scalar coordinate construction and bounds checks form the control plane;
/// every output group is packed and stored through a native SIMD byte block.
pub fn simd_transform(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill,
        fill_is_none,
        palette_fill: _,
        ..
    } = op
    else {
        return Err(PilError::ValueError("expected Transform op".into()));
    };
    let luma16_supported = native_affine_luma16_transform_supported_for_image(
        img, *w, *h, method, data, *filter, mode,
    );
    let supported = match method {
        TransformMethod::Affine => {
            luma16_supported
                || native_affine_nearest_transform_supported_for_image(
                    img, *w, *h, method, data, *filter, mode,
                )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            native_projective_nearest_transform_supported_for_image(
                img, *w, *h, method, data, *filter, mode,
            )
        }
        TransformMethod::Mesh => {
            native_mesh_transform_supported_for_image(img, *w, *h, data, *filter, mode)
        }
    };
    if !supported {
        return Err(simd_unsupported("Transform"));
    }
    if luma16_supported {
        return simd_affine_luma16_transform_bytes(img, *w, *h, data, *fill, *filter, mode)?
            .ok_or_else(|| simd_unsupported("Transform"));
    }
    match method {
        TransformMethod::Affine => match filter {
            ResampleFilter::Nearest => {
                simd_affine_nearest_transform_bytes(img, *w, *h, data, *fill, mode)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_affine_bilinear_transform_bytes(img, *w, *h, data, *fill, mode)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Perspective => match filter {
            ResampleFilter::Nearest => {
                simd_projective_nearest_transform_bytes(img, *w, *h, data, *fill, mode, false)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_projective_bilinear_transform_bytes(img, *w, *h, data, *fill, mode, false)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Quad => match filter {
            ResampleFilter::Nearest => {
                simd_projective_nearest_transform_bytes(img, *w, *h, data, *fill, mode, true)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_projective_bilinear_transform_bytes(img, *w, *h, data, *fill, mode, true)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Mesh => {
            simd_mesh_transform_bytes(img, *w, *h, data, *fill, *fill_is_none, mode)?
                .ok_or_else(|| simd_unsupported("Transform"))
        }
    }
}
