//! SSE2-selected native-L strategy for ImageOps.colorize.

use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    gray: &[u8],
    width: u32,
    height: u32,
    lut: &[[u8; 256]; 3],
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::colorize::execute_native_luma(
        gray, width, height, lut,
    )
}
