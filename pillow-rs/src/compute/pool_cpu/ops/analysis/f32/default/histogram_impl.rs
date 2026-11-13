use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbaImage, mask_raw: Option<&[u8]>) -> Result<Vec<u32>, PilError> {
    crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::f32_scalar(image, mask_raw)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_with_extrema(
    image: &RgbaImage,
    mask_raw: Option<&[u8]>,
    minimum: f32,
    maximum: f32,
) -> Result<Vec<u32>, PilError> {
    crate::compute::pool_cpu::ops::analysis::helpers::histogram_samples::f32_scalar_with_extrema(
        image, mask_raw, minimum, maximum,
    )
}
