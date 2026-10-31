//! neon selected native-byte Pad kernel for ImageHsv8.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{
    dimensions::PadFrame, native_vector,
};
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn pad(
    image: &RgbImage,
    frame: PadFrame,
) -> Result<(RgbImage, u64, u64), PilError> {
    native_vector::pad(image, frame, super::super::default::pad_impl::fill(frame.color))
}
