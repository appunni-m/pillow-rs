//! NEON-selected native Luma8 Convert strategy.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(source, target)
}
