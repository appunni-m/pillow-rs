//! NEON architecture strategy for native LA Convert targets.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(source, target)
}
