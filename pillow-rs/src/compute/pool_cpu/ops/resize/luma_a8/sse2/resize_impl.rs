//! SSE2 filtered Resize entry point for the exact luma_a8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute::<2, true, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_boxed(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<2, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
