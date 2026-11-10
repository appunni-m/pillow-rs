//! SSE2-selected native-order unsigned 16-bit conversions.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::super::helpers::luma16::execute_native_vector(source, target)
}
