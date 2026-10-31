//! Same-byte UnsharpMask blending shared by exact native mode strategies.

use crate::error::PilError;

/// Blend source samples into the already allocated Gaussian output buffer.
pub(crate) fn blend_native_bytes(
    original: &[u8],
    blurred_output: &mut [u8],
    percent: i32,
    threshold: i32,
) -> Result<(), PilError> {
    if original.len() != blurred_output.len() {
        return Err(PilError::InternalError(
            "UnsharpMask source and blur buffers have different lengths".into(),
        ));
    }
    if percent == 150 {
        blend_150(original, blurred_output, threshold);
    } else {
        blend_percent(original, blurred_output, percent, threshold);
    }
    Ok(())
}

#[inline(always)]
fn blend_150(original: &[u8], blurred_output: &mut [u8], threshold: i32) {
    for (output, &source) in blurred_output.iter_mut().zip(original) {
        let source = i32::from(source);
        let difference = source - i32::from(*output);
        *output = if difference.abs() > threshold {
            clip_byte(source + adjustment_150(difference))
        } else {
            source as u8
        };
    }
}

#[inline(always)]
fn blend_percent(original: &[u8], blurred_output: &mut [u8], percent: i32, threshold: i32) {
    for (output, &source) in blurred_output.iter_mut().zip(original) {
        let source = i32::from(source);
        let difference = source - i32::from(*output);
        *output = if difference.abs() > threshold {
            clip_byte(source + difference * percent / 100)
        } else {
            source as u8
        };
    }
}

#[inline(always)]
fn adjustment_150(difference: i32) -> i32 {
    let tripled = difference * 3;
    (tripled + i32::from(tripled < 0)) >> 1
}

#[inline(always)]
fn clip_byte(value: i32) -> u8 {
    if value >= 255 {
        255
    } else if value <= 0 {
        0
    } else {
        value as u8
    }
}
