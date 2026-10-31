//! SSE2-selected Solarize strategy over native palette indices.

use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::{
    self, NativeBytePointTransform,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let bytes = native_bytes::pointwise(
        input.as_raw(),
        NativeBytePointTransform::Solarize { threshold },
    )?;
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("solarize P buffer mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(output))
}
