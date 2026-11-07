use super::*;

#[cfg(target_arch = "x86_64")]
use crate::compute::{Architecture, Backend, ExecutionContext};

pub(crate) trait F64MulAdd: Sync {
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64;

    #[inline(always)]
    fn mul_add_finite(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        self.mul_add(weight, sample, accumulator)
    }

    fn supports_finite_mul_add(&self) -> bool {
        false
    }
}

#[inline(always)]
pub(crate) fn f_resize_samples_allow_finite_fma<F: F64MulAdd>(fma: &F, samples: &[f32]) -> bool {
    fma.supports_finite_mul_add() && samples.iter().all(|sample| sample.is_finite())
}

/// Check that every input to a finite-only FMA row is finite and that each
/// ordered f64 accumulator stays well within the finite range. F-mode samples
/// are f32, so a row's sum of absolute weighted sample bounds proves every
/// intermediate f64 accumulator finite.
pub(crate) fn f_resize_coefficients_allow_finite_fma(coefficients: &FilterCoeffsF64) -> bool {
    let max_accumulator = f64::MAX * 0.5;
    let max_sample = f64::from(f32::MAX);
    coefficients.weights.iter().all(|row| {
        let mut absolute_sum = 0.0f64;
        for &weight in row {
            if !weight.is_finite() {
                return false;
            }
            absolute_sum += weight.abs() * max_sample;
            if !absolute_sum.is_finite() || absolute_sum >= max_accumulator {
                return false;
            }
        }
        true
    })
}

pub(crate) struct PortableFma;

impl F64MulAdd for PortableFma {
    #[inline(always)]
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        weight.mul_add(sample, accumulator)
    }
}

#[cfg(target_arch = "x86_64")]
pub(crate) struct X86FmaToken {
    _private: (),
}

#[cfg(target_arch = "x86_64")]
impl X86FmaToken {
    pub(crate) fn from_context(
        context: &ExecutionContext,
        implementation: Architecture,
    ) -> Option<Self> {
        if context.backend() == Backend::Cpu
            && matches!(implementation, Architecture::AvxFma | Architecture::Avx2Fma)
            && context.permits_architecture(implementation)
            && context.supports_architecture(implementation)
        {
            Some(Self { _private: () })
        } else {
            None
        }
    }

    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add_finite_operands(weight: f64, sample: f64, accumulator: f64) -> f64 {
        let mut result = accumulator;
        // SAFETY: callers hold an `X86FmaToken` created from the captured
        // execution context or explicit feature detection, both of which
        // establish that this CPU supports the scalar FMA instruction.
        unsafe {
            std::arch::asm!(
                "vfmadd231sd {result}, {weight}, {sample}",
                result = inout(xmm_reg) result,
                weight = in(xmm_reg) weight,
                sample = in(xmm_reg) sample,
                options(pure, nomem, nostack, preserves_flags),
            );
        }
        result
    }
}

#[cfg(target_arch = "x86_64")]
impl F64MulAdd for X86FmaToken {
    #[allow(unsafe_code)]
    #[inline(always)]
    fn mul_add(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        // libm and the hardware instruction can choose different NaN payload
        // bits for non-finite arithmetic. Preserve the portable reference's
        // exact float representation for those edge cases.
        if !weight.is_finite() || !sample.is_finite() || !accumulator.is_finite() {
            return weight.mul_add(sample, accumulator);
        }
        Self::mul_add_finite_operands(weight, sample, accumulator)
    }

    #[inline(always)]
    fn mul_add_finite(&self, weight: f64, sample: f64, accumulator: f64) -> f64 {
        Self::mul_add_finite_operands(weight, sample, accumulator)
    }

    #[inline(always)]
    fn supports_finite_mul_add(&self) -> bool {
        true
    }
}
