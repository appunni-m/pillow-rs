//! RGB8 resampling over native interleaved samples.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

/// Resizes RGB bytes while skipping source rows proven to contain only zeros.
pub(crate) fn execute_boxed_with_sparse_rows(
    input: &RgbImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<RgbImage, PilError> {
    let bytes =
        super::super::super::helpers::byte_samples::resize_boxed_with_sparse_source_rows::<3>(
            input.as_raw(),
            input.dimensions(),
            (width, height),
            filter,
            bounds,
            sparse_source_rows,
        )?;
    RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_rgb: failed to create output buffer".into()))
}

pub(crate) fn execute(
    input: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<3>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    RgbImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_rgb: failed to create output buffer".into()))
}
