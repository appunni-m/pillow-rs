//! WASM_SIMD128 filtered Resize entry point for the exact rgbx8 mode.

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

/// Wasm SIMD128 filtered Resize into a caller-owned full-width Pad window.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_into_window(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> Result<bool, PilError> {
    super::super::super::helpers::native_byte_vector::execute_into::<4, false, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        output,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
