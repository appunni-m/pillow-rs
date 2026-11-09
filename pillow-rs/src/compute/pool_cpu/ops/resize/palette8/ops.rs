//! Exact ImagePalette8 mode dispatch for Resize.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{ImplementationSupport, ModeSupport, StrategySelection};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{GrayImage, Mode};

static RESIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::native_byte_architectures(Mode::ImagePalette8, 1, true);
static RESIZE_BOXED_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::boxed_byte_architectures(Mode::ImagePalette8, 1, true);

pub(crate) const SUPPORT: ModeSupport =
    super::super::helpers::support::native_mode_support(Mode::ImagePalette8, &RESIZE_ARCHITECTURES);
pub(crate) const BOXED_SUPPORT: ModeSupport = super::super::helpers::support::boxed_mode_support(
    Mode::ImagePalette8,
    &RESIZE_BOXED_ARCHITECTURES,
);

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
            GrayImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("resize_p: failed to create output buffer".into())
            })
        },
    )
}
