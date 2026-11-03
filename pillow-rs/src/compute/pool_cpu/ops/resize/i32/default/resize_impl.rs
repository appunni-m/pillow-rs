//! Native signed-32 sample Resize, preserving Pillow's pass-rounding boundary.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{F64MulAdd, PortableFma, i32_samples_from_le_bytes};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

use super::super::super::helpers::typed_nearest::BoxBounds;

pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
) -> Result<RgbaImage, PilError> {
    execute_with_fma(input, width, height, filter, bounds, &PortableFma)
}

pub(crate) fn execute_with_fma<F: F64MulAdd>(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
        return empty(width, height);
    }
    if bounds.is_none() && (width, height) == (source_width, source_height) {
        return Ok(input.clone());
    }
    let source_dims = CheckedDims::new_allow_empty(source_width, source_height, 4)?;
    let source = i32_samples_from_le_bytes(input.as_raw(), source_dims.total_pixels());
    if matches!(filter, ResampleFilter::Nearest) {
        return super::nearest::execute(
            &source,
            (source_width, source_height),
            (width, height),
        );
    }
    let (horizontal, vertical) = super::super::super::helpers::coefficients::for_resize(
        (source_width, source_height),
        (width, height),
        filter,
        bounds,
    );
    super::super::helpers::streaming::execute(
        &source,
        source_width,
        width,
        height,
        &horizontal,
        &vertical,
        fma,
    )
}

fn empty(width: u32, height: u32) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    RgbaImage::from_raw(width, height, dims.alloc_buffer()).ok_or_else(|| {
        PilError::ValueError("resize_i: failed to create output buffer".into())
    })
}
