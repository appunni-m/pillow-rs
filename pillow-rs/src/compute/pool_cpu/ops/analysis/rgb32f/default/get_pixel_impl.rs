use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::Rgb32FImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
// NO_PILLOW_SUPPORT: this internal high-depth identity has no Pillow GetPixel mode.
/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(
    image: &Rgb32FImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    Ok(pixel_access::components_f32_3(image, x, y))
}
