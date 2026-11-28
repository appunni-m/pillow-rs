//! SSE2-selected native bilevel-to-I;16 conversion.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::i16::execute(source, target)
}
