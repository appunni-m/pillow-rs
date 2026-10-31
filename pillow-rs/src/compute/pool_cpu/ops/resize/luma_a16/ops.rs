//! Exact typed Resize strategy for internal 16-bit luma-alpha.

// NO_PILLOW_SUPPORT: this native 16-bit mode extends Pillow's Resize modes.
use crate::raster::{DynamicImage, ImageBuffer, LumaA};

pub(crate) fn resize(
    input: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> DynamicImage {
    DynamicImage::ImageLumaA16(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    ))
}

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    architecture: crate::compute::Architecture,
    input: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    super::super::helpers::strategy::require_default_architecture(context, architecture)?;
    Ok(DynamicImage::ImageLumaA16(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    )))
}
