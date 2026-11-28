//! Portable Default strategy for ImageF32 Resize.

use crate::error::PilError;
use crate::ops::pil_resize::PortableFma;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
) -> Result<RgbaImage, PilError> {
    super::super::helpers::resize::execute_with_fma::<16, _>(
        input,
        width,
        height,
        filter,
        bounds,
        &PortableFma,
    )
}
