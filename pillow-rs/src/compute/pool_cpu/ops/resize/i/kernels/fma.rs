//! x86 scalar-FMA strategy for Pillow `I` samples.

use crate::error::PilError;
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(super) fn resize(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
    fma: &X86FmaToken,
) -> Result<DynamicImage, PilError> {
    super::default::resize_i_with_fma(image, width, height, filter, fma)
}
