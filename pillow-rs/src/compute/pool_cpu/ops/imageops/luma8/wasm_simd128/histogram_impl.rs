//! Wasm SIMD128-selected L histogram LUT kernels.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn autocontrast(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_luma_architecture(
        input, lut, false,
    )
}

pub(crate) fn equalize(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_luma_architecture(
        input, lut, true,
    )
}
