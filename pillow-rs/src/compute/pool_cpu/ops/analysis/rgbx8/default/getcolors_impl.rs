use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(maxcolors: u32, image: &RgbaImage) -> Result<GetColorsOutput, PilError> {
    getcolors::byte_bands(maxcolors, image)
}
