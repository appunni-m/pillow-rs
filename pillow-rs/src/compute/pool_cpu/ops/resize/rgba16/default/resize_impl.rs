//! Native u16 kernel for ImageRgba16.

use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Rgba};

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &ImageBuffer<Rgba<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    super::super::super::helpers::typed_u16::resize_with_alpha(input, width, height, filter, bounds)
}
