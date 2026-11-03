use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::RgbImage;

pub(crate) fn execute(
    maxcolors: u32,
    image: &RgbImage,
) -> Result<GetColorsOutput, PilError> {
    getcolors::rgb8(maxcolors, image)
}
