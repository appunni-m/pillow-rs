//! Wasm SIMD128-selected little-endian unsigned 16-bit conversions.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

pub(crate) fn execute(source: &Samples, target: Mode) -> Result<DynamicImage, PilError> {
    super::super::helpers::convert::luma16::execute_native_vector(source, target)
}
