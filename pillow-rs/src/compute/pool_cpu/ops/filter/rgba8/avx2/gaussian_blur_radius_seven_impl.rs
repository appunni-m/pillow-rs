//! RGBA radius-seven GaussianBlur orchestration for AVX2.

use super::gaussian_blur_radius_seven::Avx2Kernel;
use crate::compute::ExecutionContext;
use crate::error::PilError;
use crate::raster::DynamicImage;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter::rgba8) fn execute_gaussian_blur_radius_seven(
    image: &DynamicImage,
    sigma: f32,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::execute_radius_seven::<Avx2Kernel>(
        image,
        sigma,
        context,
        "avx2-transpose",
    )
}
