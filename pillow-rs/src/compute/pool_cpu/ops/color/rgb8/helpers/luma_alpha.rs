//! Native RGB8 to LumaA8 output kernel shared by byte-RGB source modes.

use crate::error::PilError;
use crate::raster::{Mode, RgbImage};

pub(crate) fn execute(source: &RgbImage) -> Result<(Vec<u8>, u64), PilError> {
    crate::compute::pool_cpu::ops::color::helpers::convert::native_rgb::to_luma_alpha_opaque::<3>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgb8,
    )
}
