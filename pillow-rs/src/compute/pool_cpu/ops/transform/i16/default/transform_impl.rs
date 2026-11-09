//! Native u16 Transform implementation for exact mode ImageI16.

use crate::compute::pool_cpu::ops::transform::helpers::luma16::{self, ByteOrder};
use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_default_into;
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer};

pub(crate) fn execute(
    input: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16(source) = input else {
        return Err(PilError::InternalError(
            "Transform I;16 source identity mismatch".into(),
        ));
    };
    let samples = luma16::execute(
        source.as_raw(),
        source.dimensions(),
        request,
        ByteOrder::Native,
    )?;
    let output =
        ImageBuffer::from_raw(request.width, request.height, samples).ok_or_else(|| {
            PilError::InternalError("Transform I;16 output buffer shape mismatch".into())
        })?;
    Ok(DynamicImage::ImageI16(output))
}

luma16_mode_default_into!(ImageI16, Native, "I;16");

pub(crate) fn apply_mesh_record(
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
    output_plane: &mut Option<Vec<u8>>,
) -> Result<(), PilError> {
    let (DynamicImage::ImageI16(source), DynamicImage::ImageI16(destination)) = (input, output)
    else {
        return Err(PilError::InternalError(
            "Transform I;16 MESH mode mismatch".into(),
        ));
    };
    luma16::apply_mesh_record(
        source.as_raw(),
        destination.as_mut(),
        source.dimensions(),
        request,
        ByteOrder::Native,
        box_coordinates,
        coefficients,
        source_scratch,
        output_plane,
    )
}

pub(crate) fn finish_mesh(
    output: &mut DynamicImage,
    _request: TransformRequest<'_>,
    output_plane: Option<&[u8]>,
) -> Result<(), PilError> {
    let DynamicImage::ImageI16(destination) = output else {
        return Err(PilError::InternalError(
            "Transform I;16 MESH output mismatch".into(),
        ));
    };
    luma16::finish_mesh_records(destination.as_mut(), ByteOrder::Native, output_plane)
}
