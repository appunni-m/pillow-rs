//! Native u16 kernel for the ImageI16B byte-order strategy.

use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    super::super::super::helpers::luma16::resize::<true>(input, width, height, filter, bounds)
}
