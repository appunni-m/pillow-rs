//! NEON-selected Equalize data kernel after P palette expansion.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(input: &RgbImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    super::super::super::rgb8::neon::histogram_impl::equalize(input, lut)
}
