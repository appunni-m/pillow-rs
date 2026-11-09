//! Native index/alpha reducer for exact ImagePaletteAlpha8 semantics.
//!
//! PA averages both stored bytes independently, matching Pillow's raw
//! index/alpha path. It does not premultiply alpha, and the archived SIMD
//! admission did not include PA.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    input: &crate::raster::GrayAlphaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(DynamicImage::ImagePaletteAlpha8(input.clone()));
    }
    let bytes = super::super::super::helpers::native_bytes::reduce_bytes::<2, false>(
        input.as_raw(),
        input.width(),
        input.height(),
        x_factor,
        y_factor,
    )?;
    let output = crate::raster::GrayAlphaImage::from_raw(
        input.width().div_ceil(x_factor.max(1)),
        input.height().div_ceil(y_factor.max(1)),
        bytes,
    )
    .ok_or_else(|| {
        PilError::InternalError("failed to construct ImagePaletteAlpha8 Reduce output".into())
    })?;
    Ok(DynamicImage::ImagePaletteAlpha8(output))
}
