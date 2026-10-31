//! Native u16 kernel for ImageRgb16.

use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Rgb};

pub(crate) fn execute(
    input: &ImageBuffer<Rgb<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> ImageBuffer<Rgb<u16>, Vec<u16>> {
    super::super::super::helpers::typed_u16::resize(input, width, height, filter, bounds)
}
