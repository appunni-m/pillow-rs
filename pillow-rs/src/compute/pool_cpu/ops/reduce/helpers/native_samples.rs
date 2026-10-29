//! Typed reducers that preserve each source sample and image identity.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

fn output_sample_capacity(
    width: u32,
    height: u32,
    channels: usize,
    bytes_per_sample: usize,
) -> Result<usize, PilError> {
    let bytes_per_pixel = channels.checked_mul(bytes_per_sample).ok_or_else(|| {
        PilError::DimensionError("typed reducer pixel size overflow".into())
    })?;
    let dims = CheckedDims::new(width, height, u8::try_from(bytes_per_pixel).map_err(|_| {
        PilError::DimensionError("typed reducer pixel size exceeds u8".into())
    })?)?;
    dims.total_pixels()
        .checked_mul(channels)
        .ok_or_else(|| PilError::DimensionError("typed reducer sample count overflow".into()))
}

/// Reduces unsigned 16-bit native samples with nearest, half-up integer rounding.
pub(crate) fn reduce_u16<P>(
    image: &ImageBuffer<P, Vec<u16>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let width = image.width();
    let height = image.height();
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let channels = usize::from(P::CHANNEL_COUNT);
    let capacity = output_sample_capacity(output_width, output_height, channels, 2)?;
    let source = image.as_raw();
    let mut output = Vec::with_capacity(capacity);
    for y in 0..output_height {
        let source_y = y * y_factor;
        let block_height = y_factor.min(height - source_y);
        for x in 0..output_width {
            let source_x = x * x_factor;
            let block_width = x_factor.min(width - source_x);
            let mut sums = [0u128; 4];
            for dy in 0..block_height {
                for dx in 0..block_width {
                    let index = (((source_y + dy) * width + source_x + dx) as usize) * channels;
                    for channel in 0..channels {
                        sums[channel] += u128::from(source[index + channel]);
                    }
                }
            }
            let count = u128::from(block_width) * u128::from(block_height);
            for sum in &sums[..channels] {
                let sample = (sum + count / 2) / count;
                output.push(u16::try_from(sample).expect("an average remains within u16"));
            }
        }
    }
    ImageBuffer::from_raw(output_width, output_height, output).ok_or_else(|| {
        PilError::InternalError("failed to construct typed u16 Reduce output".into())
    })
}

/// Reduces native f32 channels using f64 accumulation, without changing layout.
pub(crate) fn reduce_f32<P>(
    image: &ImageBuffer<P, Vec<f32>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<ImageBuffer<P, Vec<f32>>, PilError>
where
    P: Pixel<Subpixel = f32>,
{
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let width = image.width();
    let height = image.height();
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let channels = usize::from(P::CHANNEL_COUNT);
    let capacity = output_sample_capacity(output_width, output_height, channels, 4)?;
    let source = image.as_raw();
    let mut output = Vec::with_capacity(capacity);
    for y in 0..output_height {
        let source_y = y * y_factor;
        let block_height = y_factor.min(height - source_y);
        for x in 0..output_width {
            let source_x = x * x_factor;
            let block_width = x_factor.min(width - source_x);
            let mut sums = [0.0f64; 4];
            for dy in 0..block_height {
                for dx in 0..block_width {
                    let index = (((source_y + dy) * width + source_x + dx) as usize) * channels;
                    for channel in 0..channels {
                        sums[channel] += f64::from(source[index + channel]);
                    }
                }
            }
            let count = f64::from(block_width) * f64::from(block_height);
            output.extend(sums[..channels].iter().map(|sum| (*sum / count) as f32));
        }
    }
    ImageBuffer::from_raw(output_width, output_height, output).ok_or_else(|| {
        PilError::InternalError("failed to construct typed f32 Reduce output".into())
    })
}
