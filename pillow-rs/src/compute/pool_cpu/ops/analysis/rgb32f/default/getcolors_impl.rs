//! NO_PILLOW_SUPPORT: native-width GetColors preserves CoreOnly samples.
use crate::compute::pool_cpu::ops::analysis::helpers::getcolors::{self, GetColorsOutput};
use crate::error::PilError;
use crate::raster::Rgb32FImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(maxcolors: u32, image: &Rgb32FImage) -> Result<GetColorsOutput, PilError> {
    getcolors::f32_bands(maxcolors, image)
}
