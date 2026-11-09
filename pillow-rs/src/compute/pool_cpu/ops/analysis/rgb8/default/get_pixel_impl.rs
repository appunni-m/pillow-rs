use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::RgbImage;

/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(image: &RgbImage, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
    Ok(pixel_access::components_u8_3(image, x, y))
}

/// Reads RGB directly into the binding's fixed-size tuple without allocating.
pub(crate) fn rgb_tuple(image: &RgbImage, x: u32, y: u32) -> [u8; 3] {
    pixel_access::rgb_pixel(image, x, y)
}
