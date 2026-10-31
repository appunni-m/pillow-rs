//! sse2 selected native-byte Pad kernel for ImageLumaA8.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::{
    dimensions::PadFrame, native_vector,
};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn pad(
    image: &GrayAlphaImage,
    frame: PadFrame,
) -> Result<(GrayAlphaImage, u64, u64), PilError> {
    native_vector::pad(image, frame, super::super::default::pad_impl::fill(frame.color))
}
