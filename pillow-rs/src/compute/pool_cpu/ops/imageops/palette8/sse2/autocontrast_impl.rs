//! SSE2-selected Autocontrast strategy over native palette indices.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

// NO_PILLOW_SUPPORT: this transforms P indices directly and retains P identity.
pub(crate) fn execute(input: &GrayImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::imageops::helpers::histogram::apply_palette_architecture(
        input, lut,
    )
}
