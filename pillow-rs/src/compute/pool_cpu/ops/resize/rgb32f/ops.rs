//! Exact typed Resize strategy for internal 32-bit float RGB.

// NO_PILLOW_SUPPORT: this native float mode extends Pillow's Resize modes.
use crate::raster::{DynamicImage, Rgb32FImage};

pub(crate) fn resize(
    input: &Rgb32FImage,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> DynamicImage {
    DynamicImage::ImageRgb32F(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    ))
}

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    architecture: crate::compute::Architecture,
    input: &Rgb32FImage,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    super::super::helpers::strategy::require_default_architecture(context, architecture)?;
    Ok(DynamicImage::ImageRgb32F(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    )))
}
