//! WASM_SIMD128 filtered Resize entry point for the exact lab8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute::<3, false, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_boxed(
    input: &RgbImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<3, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
