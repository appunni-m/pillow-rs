//! Exact CMYK adapter for the shared AVX-512F pixel permutation.

use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    let (width, height, raw) =
        crate::compute::pool_cpu::ops::transpose::helpers::avx512f::transpose_native32(
            image.as_raw(),
            image.width(),
            image.height(),
            *method,
        )?;
    let output = super::Input::from_raw(width, height, raw)
        .ok_or_else(|| PilError::InternalError("CMYK AVX-512F transpose shape mismatch".into()))?;
    Ok(DynamicImage::ImageCmyk8(output))
}
