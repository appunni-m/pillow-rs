//! SSE2 filtered Resize entry point for the exact rgba8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute::<4, true, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
pub(crate) fn execute_boxed(
    input: &RgbaImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<4, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
