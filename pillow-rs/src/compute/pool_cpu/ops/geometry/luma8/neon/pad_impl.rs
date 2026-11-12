//! neon selected native-byte Pad kernel for ImageLuma8.

use crate::compute::pool_cpu::ops::geometry::helpers::{
    pad_dimensions::PadFrame, pad_native_vector as native_vector,
};
use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn pad(image: &GrayImage, frame: PadFrame) -> Result<(GrayImage, u64, u64), PilError> {
    native_vector::pad(
        image,
        frame,
        super::super::default::pad_impl::fill(frame.color),
    )
}
