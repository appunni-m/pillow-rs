//! NEON-selected Solarize kernel for native RGB samples.

use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::{
    self, NativeBytePointTransform,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(input: &RgbImage, threshold: u8) -> Result<DynamicImage, PilError> {
    let bytes = native_bytes::pointwise(
        input.as_raw(),
        NativeBytePointTransform::Solarize { threshold },
    )?;
    let output = RgbImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("solarize RGB buffer mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(output))
}
