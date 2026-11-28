use crate::compute::pool_cpu::ops::analysis::helpers::pixel_access;
use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::RgbImage;

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
/// Reads one pixel from this exact mode's native carrier.
pub(crate) fn execute(image: &RgbImage, x: u32, y: u32) -> Result<FormattedPixelValue, PilError> {
    let FormattedPixelValue::Components(mut components) =
        pixel_access::components_u8_3(image, x, y)
    else {
        return Err(PilError::InternalError(
            "LAB GetPixel strategy received a non-component pixel".into(),
        ));
    };
    components[1] = components[1].wrapping_sub(128);
    components[2] = components[2].wrapping_sub(128);
    Ok(FormattedPixelValue::Components(components))
}
