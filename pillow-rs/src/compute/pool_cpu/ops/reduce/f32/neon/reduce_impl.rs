//! NEON implementation for exact ImageF32 Reduce.

use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

const OUTPUT_LANES: usize = 8;
const NEON_LANES: usize = 4;

// VERIFY: source=reviewed@2026-11-13; parity=pass@f32-reduce-neon-pillow; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let output_width = input.width().div_ceil(x_factor.max(1));
    let output_height = input.height().div_ceil(y_factor.max(1));
    if x_factor == 2
        && y_factor == 2
        && input.width().is_multiple_of(2)
        && input.height().is_multiple_of(2)
        && (output_width as usize).is_multiple_of(OUTPUT_LANES)
    {
        return reduce_even_two_by_two(input, output_width, output_height);
    }
    super::super::helpers::vector::execute(input, x_factor, y_factor)
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@f32-reduce-neon-pillow; benchmark=pending
fn reduce_even_two_by_two(
    input: &RgbaImage,
    output_width: u32,
    output_height: u32,
) -> Result<DynamicImage, PilError> {
    let source = super::super::helpers::storage::source_samples(input)?;
    let source_width = usize::try_from(input.width()).map_err(|_| {
        PilError::InternalError("ImageF32 Reduce source width does not fit usize".into())
    })?;
    let output_capacity =
        super::super::helpers::storage::output_capacity(output_width, output_height)?;
    let mut output = Vec::with_capacity(output_capacity);
    for output_y in 0..output_height as usize {
        let source_top = output_y * 2 * source_width;
        let source_bottom = source_top + source_width;
        for output_x in (0..output_width as usize).step_by(OUTPUT_LANES) {
            let values = reduce_row(&source, source_top, source_bottom, output_x)?;
            super::super::helpers::storage::append_samples(&mut output, &values);
        }
    }
    super::super::helpers::storage::finish_output(output_width, output_height, output)
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@f32-reduce-neon-pillow; benchmark=pending
#[allow(unsafe_code)]
fn reduce_row(
    source: &[f32],
    source_top: usize,
    source_bottom: usize,
    output_x: usize,
) -> Result<[f32; OUTPUT_LANES], PilError> {
    let source_x = output_x
        .checked_mul(2)
        .ok_or_else(|| PilError::InternalError("ImageF32 Reduce source index overflow".into()))?;
    let top = source_top
        .checked_add(source_x)
        .ok_or_else(|| PilError::InternalError("ImageF32 Reduce source index overflow".into()))?;
    let bottom = source_bottom
        .checked_add(source_x)
        .ok_or_else(|| PilError::InternalError("ImageF32 Reduce source index overflow".into()))?;
    if top
        .checked_add(OUTPUT_LANES * 2)
        .is_none_or(|end| end > source.len())
        || bottom
            .checked_add(OUTPUT_LANES * 2)
            .is_none_or(|end| end > source.len())
    {
        return Err(PilError::InternalError(
            "ImageF32 Reduce NEON block exceeds source storage".into(),
        ));
    }
    let mut quartets = [0.0f32; OUTPUT_LANES];
    // SAFETY: The caller selects this kernel only for complete 2x2 blocks and
    // eight output lanes. The bounds above verify both source-row loads. The
    // structure loads accept the f32 slice's alignment, and each store writes
    // four initialized values into the local eight-element array.
    unsafe {
        use core::arch::aarch64::{vaddq_f32, vld2q_f32, vst1q_f32};

        let top_first = vld2q_f32(source.as_ptr().add(top));
        let top_second = vld2q_f32(source.as_ptr().add(top + NEON_LANES * 2));
        let bottom_first = vld2q_f32(source.as_ptr().add(bottom));
        let bottom_second = vld2q_f32(source.as_ptr().add(bottom + NEON_LANES * 2));
        let first = vaddq_f32(
            vaddq_f32(vaddq_f32(top_first.0, top_first.1), bottom_first.0),
            bottom_first.1,
        );
        let second = vaddq_f32(
            vaddq_f32(vaddq_f32(top_second.0, top_second.1), bottom_second.0),
            bottom_second.1,
        );
        vst1q_f32(quartets.as_mut_ptr(), first);
        vst1q_f32(quartets.as_mut_ptr().add(NEON_LANES), second);
    }
    Ok(quartets.map(|quartet| {
        let mut sum = 0.0f64;
        sum += f64::from(quartet);
        (sum * 0.25) as f32
    }))
}
