//! Scalar byte-LUT mapping shared by exact byte modes.

/// Applies one channel-major 256-entry table per native byte band.
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
    for (channel, coefficients) in affine.iter_mut().enumerate() {
        let start = channel * 256;
        let Some(pair) =
            crate::compute::byte_lut_affine_coefficients(&lut[start..start + 256])
        else {
            all_affine = false;
            break;
        };
        *coefficients = pair;
    }

    let mut output = Vec::with_capacity(source.len());
    if all_affine {
        map_affine(source, &mut output, &affine);
        return Some(output);
    }
    map_tables::<CHANNELS>(source, lut, &mut output)?;
    Some(output)
}

fn map_affine<const CHANNELS: usize>(
    source: &[u8],
    output: &mut Vec<u8>,
    affine: &[(u8, u8); CHANNELS],
) {
    for pixel in source.chunks_exact(CHANNELS) {
        for (channel, &sample) in pixel.iter().enumerate() {
            let (multiplier, offset) = affine[channel];
            output.push(sample.wrapping_mul(multiplier).wrapping_add(offset));
        }
    }
}

fn map_tables<const CHANNELS: usize>(
    source: &[u8],
    lut: &[u8],
    output: &mut Vec<u8>,
) -> Option<()> {
    match CHANNELS {
        1 => output.extend(source.iter().map(|&sample| lut[usize::from(sample)])),
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
    Some(())
}
