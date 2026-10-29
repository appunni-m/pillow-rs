//! Wasm SIMD128 Posterize kernel for Pillow's native L mode.

use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::{
    self, NativeBytePointTransform,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(input: &GrayImage, bits: u8) -> Result<DynamicImage, PilError> {
    let bytes = native_bytes::pointwise(
        input.as_raw(),
        NativeBytePointTransform::Posterize { bits },
    )?;
    let output = GrayImage::from_raw(input.width(), input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("posterize L buffer mismatch".into()))?;
    Ok(DynamicImage::ImageLuma8(output))
}
