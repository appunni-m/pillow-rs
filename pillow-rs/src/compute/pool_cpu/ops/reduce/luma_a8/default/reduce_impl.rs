//! Native byte reducer for this exact raster identity.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::GrayAlphaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImageLumaA8(input.clone()));
    }
    let bytes = super::super::super::helpers::native_bytes::reduce_bytes::<2, true>(
        input.as_raw(), input.width(), input.height(), x_factor, y_factor,
    )?;
    let output = crate::raster::GrayAlphaImage::from_raw(
        input.width().div_ceil(x_factor.max(1)),
        input.height().div_ceil(y_factor.max(1)),
        bytes,
    )
    .ok_or_else(|| PilError::InternalError("failed to construct native byte Reduce output".into()))?;
    Ok(DynamicImage::ImageLumaA8(output))
}
