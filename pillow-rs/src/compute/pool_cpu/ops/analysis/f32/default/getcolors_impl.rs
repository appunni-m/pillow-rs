use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    maxcolors: u32,
    image: &RgbaImage,
) -> Result<GetColorsOutput, PilError> {
    getcolors::scalar_f32(maxcolors, image)
}
