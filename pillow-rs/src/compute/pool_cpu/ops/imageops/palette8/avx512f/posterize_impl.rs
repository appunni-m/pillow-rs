//! AVX-512F Posterize strategy over native palette indices.

// NO_PILLOW_SUPPORT: Pillow transforms P palette entries; this Rust route transforms indices.

use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::avx512f;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(input: &GrayImage, bits: u8) -> Result<DynamicImage, PilError> {
    let bytes = avx512f::posterize(input.as_raw(), bits)?;
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("posterize P buffer mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(output))
}
