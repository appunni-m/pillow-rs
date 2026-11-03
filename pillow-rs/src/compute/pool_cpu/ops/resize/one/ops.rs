//! Exact one-byte-per-pixel mode dispatch for Resize.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

pub(crate) const fn effective_filter(_requested: ResampleFilter) -> ResampleFilter {
    ResampleFilter::Nearest
}

pub(crate) fn resize(
    context: &ExecutionContext,
    selection: &StrategySelection,
    image: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayImage, PilError> {
    let architecture = selection.implementation.architecture;
    super::super::helpers::strategy::resize_byte_nearest::<_, 1>(
        context,
        selection,
        image.as_raw(),
        image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || super::default::resize_impl::execute(image, width, height, filter, bounds),
        |bytes| {
            let mut output = GrayImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("one-byte Resize failed to create its output plane".into())
            })?;
            for pixel in output.pixels_mut() {
                pixel[0] = if pixel[0] >= 128 { 255 } else { 0 };
            }
            Ok(output)
        },
    )
}
