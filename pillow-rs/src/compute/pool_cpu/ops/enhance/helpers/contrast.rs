//! Native-byte Contrast blending shared by exact mode strategies.

use crate::compute::helpers::enhance::ContrastBase;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn execute(
    source: &[u8],
    base: &ContrastBase,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    let channels = base.channels;
    if !(1..=4).contains(&channels)
        || base.alpha.is_some_and(|alpha| alpha >= channels)
        || source.len() % channels != 0
    {
        return Err(PilError::InternalError(
            "Contrast source bytes did not match its exact mode".into(),
        ));
    }

    let factor = factor as f32;
    if factor.is_finite() {
        let mut lookup = Vec::with_capacity(channels * 256);
        for channel in 0..channels {
            let midpoint = f32::from(base.values[channel]);
            for sample in 0..=u8::MAX {
                if base.alpha == Some(channel) {
                    lookup.push(sample);
                } else {
                    lookup.push(
                        factor
                            .mul_add(f32::from(sample) - midpoint, midpoint)
                            .clamp(0.0, 255.0) as u8,
                    );
                }
            }
        }
        let mut output = Vec::with_capacity(source.len());
        for pixel in source.chunks_exact(channels) {
            for (channel, &sample) in pixel.iter().enumerate() {
                output.push(lookup[channel * 256 + usize::from(sample)]);
            }
        }
        return Ok(output);
    }

    let mut output = Vec::with_capacity(source.len());
    for pixel in source.chunks_exact(channels) {
        for (channel, &sample) in pixel.iter().enumerate() {
            let original = f32::from(sample);
            let midpoint = if base.alpha == Some(channel) {
                original
            } else {
                f32::from(base.values[channel])
            };
            output.push(
                factor
                    .mul_add(original - midpoint, midpoint)
                    .clamp(0.0, 255.0) as u8,
            );
        }
    }
    Ok(output)
}

pub(crate) fn execute_image<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    base: &ContrastBase,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let output = execute(source.as_raw(), base, factor)?;
    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("Contrast output dimensions were invalid".into())
    })
}
