//! Exact typed Resize strategy for internal 16-bit RGB.

// NO_PILLOW_SUPPORT: this native 16-bit mode extends Pillow's Resize modes.
use crate::compute::registry::support::{ModeSupport, OperationId, StrategySelection};
use crate::raster::{DynamicImage, ImageBuffer, Mode, Rgb};

pub(crate) const SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::Resize,
    Mode::ImageRgb16,
    super::super::helpers::support::INPUTS,
);
pub(crate) const BOXED_SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::ResizeBoxed,
    Mode::ImageRgb16,
    super::super::helpers::support::INPUTS,
);

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    selection: &StrategySelection,
    input: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    super::super::helpers::strategy::require_default_architecture(context, selection)?;
    Ok(DynamicImage::ImageRgb16(
        super::default::resize_impl::execute(input, width, height, filter, bounds),
    ))
}
