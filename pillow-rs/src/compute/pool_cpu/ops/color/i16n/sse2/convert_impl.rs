//! SSE2-selected native-order unsigned 16-bit conversions.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::super::helpers::convert::luma16::execute_native_vector(source, target)
}
