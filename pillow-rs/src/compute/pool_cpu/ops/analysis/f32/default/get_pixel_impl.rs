use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::RgbaImage;

/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(
    image: &RgbaImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    pixel_access::scalar_f32(image, x, y)
}
