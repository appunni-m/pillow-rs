//! Scalar byte-LUT mapping shared by exact byte modes.

use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::DynamicImage;

/// Restores the omitted lanes written by Pillow's mode-specific Point loops.
/// Point allocates clean output storage and writes only its declared bands, so
/// compact-mode padding/intervening lanes remain zero in the result.
pub(crate) fn initialize_point_hidden_lanes(output: &mut DynamicImage) -> Result<(), PilError> {
    let Some(lane_count) = output.mode().hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = (output.width() as usize)
        .checked_mul(output.height() as usize)
        .ok_or_else(|| PilError::InternalError("Eval output dimensions overflowed".into()))?;
    if pixel_count == 0 {
        return Ok(());
    }
    let lanes = HiddenPixelLanes::constant(lane_count, pixel_count, [0; 2])
        .ok_or_else(|| PilError::InternalError("Eval mode had an invalid hidden-lane layout".into()))?;
    if output.replace_hidden_pixel_lanes(Some(lanes)) {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "Eval output could not retain its hidden physical lanes".into(),
        ))
    }
}

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
