//! NO_PILLOW_SUPPORT: native-width GetColors preserves CoreOnly samples.
use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::Rgba32FImage;

pub(crate) fn execute(maxcolors: u32, image: &Rgba32FImage) -> Result<GetColorsOutput, PilError> {
    getcolors::f32_bands(maxcolors, image)
}
