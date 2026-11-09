use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(maxcolors: u32, image: &GrayAlphaImage) -> Result<GetColorsOutput, PilError> {
    getcolors::byte_bands(maxcolors, image)
}
