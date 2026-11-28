use crate::error::PilError;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(image: &RgbaImage) -> Result<RgbaImage, PilError> {
    super::super::super::helpers::vector::execute_invert_avx512f(image)
}
