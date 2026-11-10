//! Portable streamed 2x2 ImageI32 reducing-gap implementation.

use crate::error::PilError;
use crate::ops::pil_resize::PortableSeparateMulAdd;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    input: &RgbaImage,
    output: (u32, u32),
    filter: ResampleFilter,
) -> Result<RgbaImage, PilError> {
    super::super::helpers::streaming::execute(input, output, filter, &PortableSeparateMulAdd)
}
