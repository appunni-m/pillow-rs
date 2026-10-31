//! Exact ImagePalette8 mode dispatch for Resize.

use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayImage;

pub(crate) fn resize(
    context: &ExecutionContext,
    architecture: Architecture,
    image: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayImage, PilError> {
    super::super::helpers::strategy::resize_byte_nearest::<_, 1>(
        context,
        architecture,
        image.as_raw(),
        image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || super::default::resize_impl::execute(image, width, height, filter, bounds),
        |bytes| {
            GrayImage::from_raw(width, height, bytes)
                .ok_or_else(|| PilError::ValueError("resize_p: failed to create output buffer".into()))
        },
    )
}
