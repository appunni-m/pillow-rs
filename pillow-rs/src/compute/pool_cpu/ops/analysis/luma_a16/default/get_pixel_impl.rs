use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::{ImageBuffer, LumaA};

// NO_PILLOW_SUPPORT: this internal high-depth identity has no Pillow GetPixel mode.
/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(
    image: &ImageBuffer<LumaA<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    Ok(pixel_access::components_u16_2(image, x, y))
}
