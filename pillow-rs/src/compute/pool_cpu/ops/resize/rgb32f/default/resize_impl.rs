//! Native f32 kernel for ImageRgb32F.

use crate::pipeline::ResampleFilter;
use crate::raster::Rgb32FImage;

pub(crate) fn execute(
    input: &Rgb32FImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> Rgb32FImage {
    super::super::super::helpers::typed_f32::resize(input, width, height, filter, bounds)
}
