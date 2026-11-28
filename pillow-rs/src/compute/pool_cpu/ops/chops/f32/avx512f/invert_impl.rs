//! AVX-512F ImageChops inversion for the exact float32 ImageF32 mode.

use crate::error::PilError;
use crate::raster::RgbaImage;

// NO_PILLOW_SUPPORT: ImageChops F inversion extends Pillow with native sample-byte inversion.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(input: &RgbaImage) -> Result<RgbaImage, PilError> {
    super::super::super::helpers::vector::execute_invert_avx512f(input)
}
