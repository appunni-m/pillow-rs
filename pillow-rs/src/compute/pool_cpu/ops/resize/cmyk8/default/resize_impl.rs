//! Resize C/M/Y/K samples in their native four-byte carrier.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

use super::filtered;

pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbaImage, PilError> {
    let source_size = input.dimensions();
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, 4)?;
    if input.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "ImageCmyk8 Resize received an invalid source buffer".into(),
        ));
    }
    if bounds.is_some()
        || matches!(filter, ResampleFilter::Nearest)
        || source_size.0 == 0
        || source_size.1 == 0
        || width == 0
        || height == 0
        || source_size == (width, height)
    {
        return resized_with_helper(input, width, height, filter, bounds);
    }

    CheckedDims::new(width, height, 4)?;
    let horizontal = precompute_coeffs(width, source_size.0, filter);
    let vertical = precompute_coeffs(height, source_size.1, filter);
    if let Some(bytes) = filtered::execute(
        input.as_raw(),
        source_size,
        (width, height),
        &horizontal,
        &vertical,
    ) {
        return RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
            PilError::ValueError("resize_cmyk: failed to create output buffer".into())
        });
    }
    let bytes = super::super::super::helpers::byte_samples::filtered_standard_with_coefficients::<4>(
        input.as_raw(),
        source_size,
        (width, height),
        &horizontal,
        &vertical,
    )?;
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_cmyk: failed to create output buffer".into()))
}

fn resized_with_helper(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbaImage, PilError> {
    let bytes = super::super::super::helpers::byte_samples::resize::<4>(
        input.as_raw(),
        input.dimensions(),
        (width, height),
        filter,
        bounds,
    )?;
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_cmyk: failed to create output buffer".into()))
}
