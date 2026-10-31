//! NEON filtered Resize entry point for the exact rgb8 mode.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

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

/// Filtered Resize that skips source rows proven to be all zero.
pub(crate) fn execute_with_sparse_rows(
    input: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_with_sparse_rows::<3, false, true>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        sparse_source_rows,
    )
}

/// Boxed filtered Resize entry point for this exact mode and architecture.
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

/// Boxed filtered Resize that skips source rows proven to be all zero.
pub(crate) fn execute_boxed_with_sparse_rows(
    input: &RgbImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::native_byte_vector::execute_boxed_with_sparse_rows::<3, false>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        bounds,
        filter,
        sparse_source_rows,
    )
}
