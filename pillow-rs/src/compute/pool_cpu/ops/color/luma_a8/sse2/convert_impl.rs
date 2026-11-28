//! SSE2 architecture strategy for native LA Convert targets.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::vector::execute(source, target)
}
