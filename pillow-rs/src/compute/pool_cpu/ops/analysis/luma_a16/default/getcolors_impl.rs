//! NO_PILLOW_SUPPORT: native-width GetColors preserves CoreOnly samples.
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image;
use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;

pub(crate) fn execute(maxcolors: u32, image: &LumaA16Image) -> Result<GetColorsOutput, PilError> {
    getcolors::u16_bands(maxcolors, image)
}
