//! Resample premultiplied luma and alpha samples without another alpha transform.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayAlphaImage;

pub(crate) fn execute(
    input: &GrayAlphaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayAlphaImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<2>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    GrayAlphaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::ValueError("resize_la_premultiplied: failed to create output buffer".into())
    })
}
