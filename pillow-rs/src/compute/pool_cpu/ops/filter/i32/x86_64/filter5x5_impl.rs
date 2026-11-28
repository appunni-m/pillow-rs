//! x86_64 baseline sequence for I-mode Filter5x5.

use super::super::default::filter5x5_impl::{F32MulAdd, execute_with_fma};
use crate::error::PilError;
use crate::raster::DynamicImage;

struct SeparateMulAdd;

impl F32MulAdd for SeparateMulAdd {
    #[inline(always)]
    fn mul_add(&self, sample: f32, weight: f32, accumulator: f32) -> f32 {
        accumulator + sample * weight
    }

    #[inline(always)]
    fn kernel_row_5(&self, pixels: [f32; 5], kernel: &[f32]) -> f32 {
        let sum = pixels[0] * kernel[0];
        let sum = sum + pixels[1] * kernel[1];
        let sum = sum + pixels[2] * kernel[2];
        let sum = sum + pixels[3] * kernel[3];
        sum + pixels[4] * kernel[4]
    }

    fn supports_binomial_factorization(&self) -> bool {
        false
    }
}

// VERIFY: source=reviewed@2026-11-27; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter::i32) fn execute(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    execute_with_fma(image, kernel, scale, offset, &SeparateMulAdd).map(|(image, _)| image)
}
