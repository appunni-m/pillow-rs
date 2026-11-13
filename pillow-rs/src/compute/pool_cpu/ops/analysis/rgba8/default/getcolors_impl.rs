use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(maxcolors: u32, image: &RgbaImage) -> Result<GetColorsOutput, PilError> {
    getcolors::byte_bands(maxcolors, image)
}
