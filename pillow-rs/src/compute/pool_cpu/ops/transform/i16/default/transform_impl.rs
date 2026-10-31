//! Native u16 Transform implementation for exact mode ImageI16.

use crate::compute::pool_cpu::ops::transform::helpers::luma16::{self, ByteOrder};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, ImageBuffer};

pub(crate) fn execute(
    input: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16(source) = input else {
        return Err(PilError::InternalError("Transform I;16 source identity mismatch".into()));
    };
    let samples = luma16::execute(
        source.as_raw(),
        source.dimensions(),
        request,
        ByteOrder::Native,
    )?;
    let output = ImageBuffer::from_raw(request.width, request.height, samples)
        .ok_or_else(|| PilError::InternalError("Transform I;16 output buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageI16(output))
}
