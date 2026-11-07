//! Exact typed Resize strategy for internal 32-bit float RGBA.

// NO_PILLOW_SUPPORT: this native float mode extends Pillow's Resize modes.
use crate::compute::registry::support::StrategySelection;
use crate::raster::{DynamicImage, Rgba32FImage};

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    selection: &StrategySelection,
    input: &Rgba32FImage,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    super::super::helpers::strategy::require_default_architecture(context, selection)?;
    Ok(DynamicImage::ImageRgba32F(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    )))
}
