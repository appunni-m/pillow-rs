//! Native u16 kernel for ImageLumaA16.

use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, LumaA};

pub(crate) fn execute(
    input: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
    super::super::super::helpers::typed_u16::resize_with_alpha(input, width, height, filter, bounds)
}
