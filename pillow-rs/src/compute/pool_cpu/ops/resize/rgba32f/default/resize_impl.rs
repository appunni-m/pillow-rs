//! Native f32 kernel for ImageRgba32F.

use crate::pipeline::ResampleFilter;
use crate::raster::Rgba32FImage;

pub(crate) fn execute(
    input: &Rgba32FImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> Rgba32FImage {
    super::super::super::helpers::typed_f32::resize_with_alpha(input, width, height, filter, bounds)
}
