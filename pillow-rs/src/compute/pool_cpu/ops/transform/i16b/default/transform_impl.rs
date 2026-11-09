//! Native u16 Transform implementation for exact mode ImageI16B.

use crate::compute::pool_cpu::ops::transform::helpers::luma16::{self, ByteOrder};
use crate::compute::pool_cpu::ops::transform::helpers::luma16_mode_default_into;
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer};

pub(crate) fn execute(
    input: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16B(source) = input else {
        return Err(PilError::InternalError(
            "Transform I;16 source identity mismatch".into(),
        ));
    };
    let samples = luma16::execute(
        source.as_raw(),
        source.dimensions(),
        request,
        ByteOrder::Big,
    )?;
    let output =
        ImageBuffer::from_raw(request.width, request.height, samples).ok_or_else(|| {
            PilError::InternalError("Transform I;16 output buffer shape mismatch".into())
        })?;
    Ok(DynamicImage::ImageI16B(output))
}

luma16_mode_default_into!(ImageI16B, Big, "I;16B");

pub(crate) fn apply_mesh_record(
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
    output_plane: &mut Option<Vec<u8>>,
) -> Result<(), PilError> {
    let (DynamicImage::ImageI16B(source), DynamicImage::ImageI16B(destination)) = (input, output)
    else {
        return Err(PilError::InternalError(
            "Transform I;16B MESH mode mismatch".into(),
        ));
    };
    luma16::apply_mesh_record(
        source.as_raw(),
        destination.as_mut(),
        source.dimensions(),
        request,
        ByteOrder::Big,
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
    let DynamicImage::ImageI16B(destination) = output else {
        return Err(PilError::InternalError(
            "Transform I;16B MESH output mismatch".into(),
        ));
    };
    luma16::finish_mesh_records(destination.as_mut(), ByteOrder::Big, output_plane)
}
