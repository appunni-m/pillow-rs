use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::GrayImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(image: &GrayImage, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
    Ok(pixel_access::scalar_u8(image, x, y))
}
