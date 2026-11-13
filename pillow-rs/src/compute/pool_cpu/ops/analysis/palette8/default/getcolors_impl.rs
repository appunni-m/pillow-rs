use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::GrayImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(maxcolors: u32, image: &GrayImage) -> Result<GetColorsOutput, PilError> {
    getcolors::luma8(maxcolors, image)
}
