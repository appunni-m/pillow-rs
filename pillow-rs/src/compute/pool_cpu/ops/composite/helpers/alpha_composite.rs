//! Pillow's native fixed-point AlphaComposite pixel arithmetic.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

pub(crate) fn apply<const CHANNELS: usize>(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
    mode_name: &'static str,
) -> Result<Vec<u8>, PilError> {
    let channels = u8::try_from(CHANNELS)
        .map_err(|_| PilError::InternalError("AlphaComposite channel count is invalid".into()))?;
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(format!(
            "{mode_name} AlphaComposite source storage did not match its dimensions"
        )));
    }

    let mut output = destination.to_vec();
    for (source_pixel, destination_pixel) in source
        .chunks_exact(CHANNELS)
        .zip(output.chunks_exact_mut(CHANNELS))
    {
        alpha_composite_pixel::<CHANNELS>(source_pixel, destination_pixel);
    }
    Ok(output)
}

#[inline]
pub(crate) fn alpha_composite_pixel<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
) {
    let source_alpha = u32::from(source[CHANNELS - 1]);
    if source_alpha == 0 {
        return;
    }

    let destination_alpha = u32::from(destination[CHANNELS - 1]);
    let blend = destination_alpha * (255 - source_alpha);
    let output_alpha_255 = source_alpha * 255 + blend;
    let source_coefficient = source_alpha * 255 * 255 * (1 << 7) / output_alpha_255;
    let destination_coefficient = (255 << 7) - source_coefficient;
    for channel in 0..CHANNELS - 1 {
        let blended = u32::from(source[channel]) * source_coefficient
            + u32::from(destination[channel]) * destination_coefficient;
        destination[channel] = (shift_for_div255(blended + (0x80 << 7)) >> 7) as u8;
    }
    destination[CHANNELS - 1] = shift_for_div255(output_alpha_255 + 0x80) as u8;
}

#[inline]
fn shift_for_div255(value: u32) -> u32 {
    ((value >> 8) + value) >> 8
}
