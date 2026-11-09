//! wasm_simd128 selected native-byte Pad kernel for ImageRgbx8.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{dimensions::PadFrame, native_vector};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn pad(image: &RgbaImage, frame: PadFrame) -> Result<(RgbaImage, u64, u64), PilError> {
    native_vector::pad(
        image,
        frame,
        super::super::default::pad_impl::fill(frame.color),
    )
}
