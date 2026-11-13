//! NO_PILLOW_SUPPORT: native-width GetColors preserves CoreOnly samples.
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image;
use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(maxcolors: u32, image: &Rgb16Image) -> Result<GetColorsOutput, PilError> {
    getcolors::u16_bands(maxcolors, image)
}
