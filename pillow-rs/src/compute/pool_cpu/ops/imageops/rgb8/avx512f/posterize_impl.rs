use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::avx512f;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(input: &RgbImage, bits: u8) -> Result<DynamicImage, PilError> {
    let bytes = avx512f::posterize(input.as_raw(), bits)?;
    let output = RgbImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("posterize RGB buffer mismatch".into()))?;
    Ok(DynamicImage::ImageRgb8(output))
}
