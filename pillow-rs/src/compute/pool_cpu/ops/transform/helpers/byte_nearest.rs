//! Shared nearest-sample routing for exact native byte-mode strategies.

use crate::compute::pool_cpu::ops::transform::helpers::{
    vector_affine_nearest, vector_projective_nearest,
};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};

pub(crate) fn execute<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
) -> Result<Vec<u8>, PilError> {
    validate_request(request)?;
    let output_len = (request.width as usize)
        .checked_mul(request.height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("transform image is too large".into()))?;
    let mut output = vec![0; output_len];
    execute_into::<CHANNELS>(source, source_dimensions, request, &mut output)?;
    Ok(output)
}

pub(crate) fn execute_into<const CHANNELS: usize>(
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    output: &mut [u8],
) -> Result<(), PilError> {
    validate_request(request)?;
    if !matches!(request.filter, ResampleFilter::Nearest) {
        return Err(PilError::InternalError(
            "native-byte vector Transform received a non-nearest request".into(),
        ));
    }
    let fill = request.fill.unwrap_or(if CHANNELS == 2 || CHANNELS == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    });
    let fill = [fill.0, fill.1, fill.2, fill.3];
    match request.method {
        TransformMethod::Affine => {
            let [a, b, c, d, e, f] = request.data else {
                return Err(PilError::InternalError(
                    "affine Transform coefficients changed after admission".into(),
                ));
            };
            vector_affine_nearest::execute_into::<CHANNELS>(
                source,
                source_dimensions,
                (request.width, request.height),
                [*a, *b, *c, *d, *e, *f],
                fill,
                output,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            vector_projective_nearest::execute_into::<CHANNELS>(
                source,
                source_dimensions,
                (request.width, request.height),
                request.data,
                fill,
                matches!(request.method, TransformMethod::Quad),
                output,
            )
        }
        TransformMethod::Mesh => Err(PilError::InternalError(
            "vector Transform received a mesh request without a declared kernel".into(),
        )),
    }
}

fn validate_request(request: TransformRequest<'_>) -> Result<(), PilError> {
    if !matches!(request.filter, ResampleFilter::Nearest) {
        return Err(PilError::InternalError(
            "native-byte vector Transform received a non-nearest request".into(),
        ));
    }
    if matches!(request.method, TransformMethod::Mesh) {
        return Err(PilError::InternalError(
            "vector Transform received a mesh request without a declared kernel".into(),
        ));
    }
    if matches!(request.method, TransformMethod::Affine) && request.data.len() != 6 {
        return Err(PilError::InternalError(
            "affine Transform coefficients changed after admission".into(),
        ));
    }
    Ok(())
}
