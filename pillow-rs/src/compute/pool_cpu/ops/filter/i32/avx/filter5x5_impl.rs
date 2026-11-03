//! Scalar FMA ring strategy for the exact normalized I-mode binomial kernel.

// NO_PILLOW_SUPPORT: Pillow's Kernel contract excludes I-mode; this is a native
// typed core extension that preserves its existing I32 representation.

use super::super::default::{execute_with_fma, F32MulAdd};
use crate::error::PilError;
use crate::raster::DynamicImage;

struct X86FmaToken;

impl F32MulAdd for X86FmaToken {
    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add(&self, sample: f32, weight: f32, accumulator: f32) -> f32 {
        let mut result = accumulator;
        // SAFETY: this token is instantiated only by the AVX/FMA strategy
        // selected after ExecutionContext validated both host features. The
        // scalar operands use XMM registers and the instruction accesses no memory.
        unsafe {
            std::arch::asm!(
                "vfmadd231ss {result}, {weight}, {sample}",
                result = inout(xmm_reg) result,
                weight = in(xmm_reg) weight,
                sample = in(xmm_reg) sample,
                options(pure, nomem, nostack, preserves_flags),
            );
        }
        result
    }
}

pub(in crate::compute::pool_cpu::ops::filter::i32) fn execute(
    image: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> Result<DynamicImage, PilError> {
    let (image, identity) = execute_with_fma(image, kernel, scale, offset, &X86FmaToken)?;
    if identity {
        crate::compute::record_pipeline_operation_path("scalar-control");
    }
    Ok(image)
}
