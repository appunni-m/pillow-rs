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
        return Err(PilError::InternalError(
            "Transform I source identity mismatch".into(),
        ));
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
    let output = RgbaImage::from_raw(request.width, request.height, bytes).ok_or_else(|| {
        PilError::InternalError("Transform I output buffer shape mismatch".into())
    })?;
    Ok(DynamicImage::ImageI32(output))
}

pub(crate) fn execute_into(
    input: &DynamicImage,
    request: TransformRequest<'_>,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    let (DynamicImage::ImageI32(source), DynamicImage::ImageI32(_)) = (input, &*output) else {
        return Err(PilError::InternalError(
            "Transform I destination mode mismatch".into(),
        ));
    };
    if matches!(request.method, TransformMethod::Mesh) {
        return Err(PilError::InternalError(
            "Transform MESH must use the record accumulator".into(),
        ));
    }
    let destination = output.as_bytes_mut().ok_or_else(|| {
        PilError::InternalError("Transform I destination storage mismatch".into())
    })?;
    if matches!(request.filter, ResampleFilter::Nearest) {
        return byte::execute_into::<4, true, false>(
            input,
            request,
            byte::AffineNearestKernel::Generic,
            destination,
        );
    }
    match request.method {
        TransformMethod::Affine => {
            let mut projective_data = request.data.to_vec();
            projective_data.extend([0.0, 0.0]);
            scalar::transform_projective_scalar32_into::<false>(
                source.as_raw(),
                source.dimensions(),
                request.width,
                request.height,
                &projective_data,
                &request.filter,
                request.fill,
                request.fill_is_none,
                false,
                destination,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            scalar::transform_projective_scalar32_into::<false>(
                source.as_raw(),
                source.dimensions(),
                request.width,
                request.height,
                request.data,
                &request.filter,
                request.fill,
                request.fill_is_none,
                matches!(request.method, TransformMethod::Quad),
                destination,
            )
        }
        TransformMethod::Mesh => unreachable!(),
    }
}

pub(crate) fn apply_mesh_record(
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
) -> Result<(), PilError> {
    let (DynamicImage::ImageI32(source), DynamicImage::ImageI32(_)) = (input, &*output) else {
        return Err(PilError::InternalError(
            "Transform I MESH mode mismatch".into(),
        ));
    };
    let output = output.as_bytes_mut().ok_or_else(|| {
        PilError::InternalError("Transform I MESH output storage mismatch".into())
    })?;
    scalar::transform_mesh_scalar32_record_with_coefficients::<false>(
        source.as_raw(),
        source.width(),
        source.height(),
        f64::from(source.width()),
        f64::from(source.height()),
        request.width,
        request.height,
        box_coordinates,
        coefficients,
        output,
        request.fill_is_none,
        &request.filter,
    );
    Ok(())
}
