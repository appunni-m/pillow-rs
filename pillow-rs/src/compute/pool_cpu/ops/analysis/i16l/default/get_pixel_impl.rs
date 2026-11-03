use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::{ImageBuffer, Luma};

/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    pixel_access::scalar_u16(image, x, y)
}
