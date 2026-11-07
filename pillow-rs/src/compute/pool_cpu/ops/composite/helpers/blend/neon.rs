//! AArch64 NEON FMA byte Blend blocks shared by exact native-byte strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

/// Runs the archived block and tail policy after the injected context selected NEON.
#[allow(unsafe_code)]
pub(crate) fn execute(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    channels: u8,
    alpha: f64,
) -> Result<DynamicImage, PilError> {
    super::execute_vector(source, other, mode, channels, alpha, |left, right, alpha| {
        // SAFETY: this entry is reachable only from a prepared NEON strategy;
        // its capability requirement checks AArch64 NEON before execution.
        unsafe { blend_block(left, right, alpha) }
    })
}

#[allow(unsafe_code)]
#[inline]
#[target_feature(enable = "neon")]
unsafe fn blend_block(left: [u8; 16], right: [u8; 16], alpha: f32) -> [u8; 16] {
    use core::arch::aarch64::{
        vandq_u32, vcvtq_f32_u32, vcvtq_s32_f32, vfmaq_f32, vdupq_n_f32, vdupq_n_u32,
        vld1q_u8, vminq_f32, vmaxq_f32, vorrq_u32, vreinterpretq_u32_s32,
        vreinterpretq_u32_u8, vreinterpretq_u8_u32,
        vshlq_n_s32, vshrq_n_u32, vst1q_u8, vsubq_f32,
    };

    // SAFETY: the target-feature boundary establishes NEON. Each local array
    // contains sixteen initialized bytes; vector accesses remain in-bounds.
    unsafe {
        let left = vreinterpretq_u32_u8(vld1q_u8(left.as_ptr()));
        let right = vreinterpretq_u32_u8(vld1q_u8(right.as_ptr()));
        let mask = vdupq_n_u32(255);
        let alpha = vdupq_n_f32(alpha);
        let zero = vdupq_n_f32(0.0);
        let maximum = vdupq_n_f32(255.0);

        macro_rules! shift_channel {
            ($value:expr, 0) => { $value };
            ($value:expr, $amount:literal) => { vshrq_n_u32($value, $amount) };
        }

        macro_rules! blend_channel {
            ($shift:tt) => {{
                let left = vandq_u32(shift_channel!(left, $shift), mask);
                let right = vandq_u32(shift_channel!(right, $shift), mask);
                let left = vcvtq_f32_u32(left);
                let right = vcvtq_f32_u32(right);
                let value = vfmaq_f32(left, vsubq_f32(right, left), alpha);
                let value = vminq_f32(maximum, vmaxq_f32(zero, value));
                vreinterpretq_u32_s32(vshlq_n_s32(
                    vcvtq_s32_f32(value),
                    $shift,
                ))
            }};
        }

        let channels = vorrq_u32(
            vorrq_u32(blend_channel!(0), blend_channel!(8)),
            vorrq_u32(blend_channel!(16), blend_channel!(24)),
        );
        let mut output = [0u8; 16];
        vst1q_u8(output.as_mut_ptr(), vreinterpretq_u8_u32(channels));
        output
    }
}
