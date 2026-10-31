//! Native one-byte Luma8 Resize with its retained narrow fixed-point kernel.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::precompute_coeffs;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

pub(crate) fn execute(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    let source_size = (source_width, source_height);
    let destination_size = (width, height);
    if bounds.is_none() && source_size == destination_size {
        return Ok(input.clone());
    }
    if is_empty(source_size, destination_size) {
        return empty(width, height);
    }
    let source_dims = CheckedDims::new(source_width, source_height, 1)?;
    let _destination_dims = CheckedDims::new(width, height, 1)?;
    if input.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "ImageLuma8 Resize received an invalid source plane".into(),
        ));
    }
    if standard_zero_source(input, bounds) || boxed_zero_source(input, bounds) {
        return empty(width, height);
    }

    let bytes = if bounds.is_none() && !matches!(filter, ResampleFilter::Nearest) {
        filtered_standard(input, source_size, destination_size, filter)?
    } else {
        super::super::super::helpers::byte_samples::resize::<1>(
            input.as_raw(),
            source_size,
            destination_size,
            filter,
            bounds,
        )?
    };
    GrayImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_l: failed to create output buffer".into()))
}

/// Resizes native L bytes into a caller-owned full-width output window.
pub(crate) fn execute_into_window(
    input: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    output: &mut [u8],
) -> bool {
    let (source_width, source_height) = input.dimensions();
    if matches!(filter, ResampleFilter::Nearest)
        || width == 0
        || height == 0
        || source_width == 0
        || source_height == 0
    {
        return false;
    }
    let horizontal = precompute_coeffs(width, source_width, filter);
    let vertical = precompute_coeffs(height, source_height, filter);
    super::filtered::execute_into(
        input.as_raw(),
        (source_width, source_height),
        (width, height),
        &horizontal,
        &vertical,
        output,
    )
    .is_some()
}

fn filtered_standard(
    input: &GrayImage,
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    let horizontal = precompute_coeffs(destination_size.0, source_size.0, filter);
    let vertical = precompute_coeffs(destination_size.1, source_size.1, filter);
    if let Some(output) = super::filtered::execute(
        input.as_raw(),
        source_size,
        destination_size,
        &horizontal,
        &vertical,
    ) {
        return Ok(output);
    }
    super::super::super::helpers::byte_samples::filtered_standard_with_coefficients::<1>(
        input.as_raw(),
        source_size,
        destination_size,
        &horizontal,
        &vertical,
    )
}

fn is_empty(source_size: (u32, u32), destination_size: (u32, u32)) -> bool {
    source_size.0 == 0
        || source_size.1 == 0
        || destination_size.0 == 0
        || destination_size.1 == 0
}

fn standard_zero_source(input: &GrayImage, bounds: Option<(f64, f64, f64, f64)>) -> bool {
    bounds.is_none()
        && !input.as_raw().is_empty()
        && input.as_raw().iter().all(|&sample| sample == 0)
}

fn boxed_zero_source(input: &GrayImage, bounds: Option<(f64, f64, f64, f64)>) -> bool {
    let (width, height) = input.dimensions();
    let Some(pixels) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    bounds.is_some()
        && pixels != 0
        && pixels <= 256 * 256
        && input.as_raw().iter().all(|&sample| sample == 0)
}

fn empty(width: u32, height: u32) -> Result<GrayImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(width, height, 1)?;
    GrayImage::from_raw(width, height, dimensions.alloc_buffer())
        .ok_or_else(|| PilError::ValueError("resize_l: failed to create output buffer".into()))
}
