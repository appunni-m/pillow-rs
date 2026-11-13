//! NEON entry point for the Luma8 filtered vector strategy.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<GrayImage, PilError> {
    super::super::helpers::vector::execute(input, width, height, filter)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_into_window(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    super::super::helpers::vector::execute_into_window(input, width, height, filter, output)
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_boxed(
    input: &GrayImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<1, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
