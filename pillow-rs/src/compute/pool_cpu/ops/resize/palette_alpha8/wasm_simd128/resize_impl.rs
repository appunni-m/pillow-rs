//! WASM_SIMD128 filtered Resize entry point for the exact palette_alpha8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute::<2, false, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
pub(crate) fn execute_boxed(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed::<2, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
    )
}
