//! SSE2 filtered Resize entry point for the exact cmyk8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute::<4, false, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
    )
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Boxed filtered Resize entry point for this exact mode and architecture.
pub(crate) fn execute_boxed(
    input: &RgbaImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<4, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
