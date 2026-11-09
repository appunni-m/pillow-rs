//! Exact typed Resize strategy for internal 32-bit float RGB.

// NO_PILLOW_SUPPORT: this native float mode extends Pillow's Resize modes.
use crate::compute::registry::support::{ModeSupport, OperationId, StrategySelection};
use crate::raster::{DynamicImage, Mode, Rgb32FImage};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Resize,
    Mode::ImageRgb32F,
    super::super::helpers::support::INPUTS,
);
pub(crate) const BOXED_SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::ResizeBoxed,
    Mode::ImageRgb32F,
    super::super::helpers::support::INPUTS,
);

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    selection: &StrategySelection,
    input: &Rgb32FImage,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    super::super::helpers::strategy::require_default_architecture(context, selection)?;
    Ok(DynamicImage::ImageRgb32F(
        super::default::resize_impl::execute(input, width, height, filter, bounds),
    ))
}
