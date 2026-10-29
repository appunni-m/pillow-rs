//! Serial native byte-LUT kernel shared by compatible mode strategies.

/// Applies one 256-entry table per interleaved native byte channel.
///
/// The exact affine fast path is retained for common linear tables. Arbitrary
/// tables use the same channel-major lookup order as the public LUT contract.
pub(crate) fn map_native_byte_lut<const CHANNELS: usize>(
    source: &[u8],
    lut: &[u8],
) -> Option<Vec<u8>> {
    if CHANNELS == 0
        || lut.len() != CHANNELS.checked_mul(256)?
        || !source.len().is_multiple_of(CHANNELS)
    {
        return None;
    }
    let mut affine = [(0u8, 0u8); CHANNELS];
    let mut all_affine = true;
    for channel in 0..CHANNELS {
        let start = channel * 256;
        let Some(coefficients) =
            crate::compute::byte_lut_affine_coefficients(&lut[start..start + 256])
        else {
            all_affine = false;
            break;
        };
        affine[channel] = coefficients;
    }
    let mut output = Vec::with_capacity(source.len());
    if all_affine {
        for pixel in source.chunks_exact(CHANNELS) {
            for (channel, &sample) in pixel.iter().enumerate() {
                let (multiplier, offset) = affine[channel];
                output.push(sample.wrapping_mul(multiplier).wrapping_add(offset));
            }
        }
        return Some(output);
    }
    match CHANNELS {
        1 => {
            for &sample in source {
                output.push(lut[usize::from(sample)]);
            }
        }
        2 => {
            for pixel in source.chunks_exact(2) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
            }
        }
        3 => {
            for pixel in source.chunks_exact(3) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
                output.push(lut[512 + usize::from(pixel[2])]);
            }
        }
        4 => {
            for pixel in source.chunks_exact(4) {
                output.push(lut[usize::from(pixel[0])]);
                output.push(lut[256 + usize::from(pixel[1])]);
                output.push(lut[512 + usize::from(pixel[2])]);
                output.push(lut[768 + usize::from(pixel[3])]);
            }
        }
        _ => return None,
    }
    Some(output)
}
