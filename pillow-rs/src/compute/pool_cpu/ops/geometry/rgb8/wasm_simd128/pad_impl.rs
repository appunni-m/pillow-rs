//! wasm_simd128 selected native-byte Pad kernel for ImageRgb8.

use crate::compute::pool_cpu::ops::geometry::helpers::{pad_dimensions::PadFrame, pad_native_vector as native_vector};
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn pad(image: &RgbImage, frame: PadFrame) -> Result<(RgbImage, u64, u64), PilError> {
    native_vector::pad(
        image,
        frame,
        super::super::default::pad_impl::fill(frame.color),
    )
}
