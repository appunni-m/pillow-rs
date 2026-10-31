//! Native u16 kernel for the ImageI16L byte-order strategy.

use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

pub(crate) fn execute(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    super::super::super::helpers::luma16::resize::<false>(input, width, height, filter, bounds)
}
