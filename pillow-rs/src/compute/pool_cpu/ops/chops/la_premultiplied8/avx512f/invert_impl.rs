use crate::error::PilError;
use crate::raster::GrayAlphaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(image: &GrayAlphaImage) -> Result<GrayAlphaImage, PilError> {
    super::super::super::helpers::vector::execute_invert_avx512f(image)
}
