//! Default Transform implementation for exact mode ImageI32.

use crate::compute::pool_cpu::ops::transform::helpers::{byte, scalar};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn execute(
    input: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(source) = input else {
        return Err(PilError::InternalError("Transform I source identity mismatch".into()));
    };
    let dimensions = source.dimensions();
    let bytes = if matches!(request.filter, ResampleFilter::Nearest) {
        byte::execute::<4, true, false>(input, request, byte::AffineNearestKernel::Generic)?
    } else {
        match request.method {
            TransformMethod::Affine => {
                let mut projective_data = request.data.to_vec();
                projective_data.extend([0.0, 0.0]);
                // Preserve the legacy ImageI32 route's fill default contract.
                scalar::transform_projective_scalar32::<false>(
                    source.as_raw(),
                    dimensions,
                    request.width,
                    request.height,
                    &projective_data,
                    &request.filter,
                    request.fill,
                    request.fill_is_none,
                    false,
                )?
            }
            TransformMethod::Perspective | TransformMethod::Quad => {
                scalar::transform_projective_scalar32::<false>(
                    source.as_raw(),
                    dimensions,
                    request.width,
                    request.height,
                    request.data,
                    &request.filter,
                    request.fill,
                    request.fill_is_none,
                    matches!(request.method, TransformMethod::Quad),
                )?
            }
            TransformMethod::Mesh => scalar::transform_mesh_scalar32::<false>(
                source.as_raw(),
                dimensions,
                request.width,
                request.height,
                request.data,
                request.fill,
                request.fill_is_none,
                &request.filter,
            )?,
        }
    };
    let output = RgbaImage::from_raw(request.width, request.height, bytes)
        .ok_or_else(|| PilError::InternalError("Transform I output buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

