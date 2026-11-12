//! sse2 selected native-byte Pad kernel for ImagePaletteAlpha8.

use crate::compute::pool_cpu::ops::geometry::helpers::{pad_dimensions::PadFrame, pad_native_vector as native_vector};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn pad(
    image: &GrayAlphaImage,
    frame: PadFrame,
) -> Result<(GrayAlphaImage, u64, u64), PilError> {
    native_vector::pad(
        image,
        frame,
        super::super::default::pad_impl::fill(frame.color),
    )
}
