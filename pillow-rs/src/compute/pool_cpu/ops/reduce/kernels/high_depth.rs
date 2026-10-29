//! Native reducers for image-slash-star storage modes beyond Pillow's modes.
//!
//! NO_PILLOW_SUPPORT: These per-channel sample reducers back only internal
//! LA16/RGB16/RGBA16/RGB32F/RGBA32F CPU strategies.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

fn output_shape(
    width: u32,
    height: u32,
    channels: usize,
    x_factor: u32,
    y_factor: u32,
) -> Result<(u32, u32, usize), PilError> {
    let output_width = width.div_ceil(x_factor.max(1));
    let output_height = height.div_ceil(y_factor.max(1));
    let sample_count = usize::try_from(output_width)
        .ok()
        .and_then(|output_width| {
            usize::try_from(output_height)
                .ok()
                .and_then(|output_height| output_width.checked_mul(output_height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::InternalError("high-depth Reduce output size overflow".into()))?;
    Ok((output_width, output_height, sample_count))
}

/// Average u16 channels independently without changing the pixel type.
pub(super) fn reduce_u16_pixels<P>(
    image: &ImageBuffer<P, Vec<u16>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<ImageBuffer<P, Vec<u16>>, PilError>
where
    P: Pixel<Subpixel = u16>,
{
    let (width, height) = image.dimensions();
    let (x_factor, y_factor) = (x_factor.max(1), y_factor.max(1));
    let channels = P::CHANNEL_COUNT as usize;
    let (output_width, output_height, output_samples) =
        output_shape(width, height, channels, x_factor, y_factor)?;
    let source = image.as_raw();
    let mut output = Vec::with_capacity(output_samples);

    for output_y in 0..output_height {
        let source_y = output_y * y_factor;
        let block_height = y_factor.min(height - source_y);
        for output_x in 0..output_width {
            let source_x = output_x * x_factor;
            let block_width = x_factor.min(width - source_x);
            let count = u128::from(block_width) * u128::from(block_height);
            for channel in 0..channels {
                let mut sum = 0u128;
                for row in 0..block_height as usize {
                    let row_start = (source_y as usize + row) * width as usize;
                    for column in 0..block_width as usize {
                        let pixel = row_start + source_x as usize + column;
                        sum += u128::from(source[pixel * channels + channel]);
                    }
                }
                output.push(((sum + count / 2) / count) as u16);
            }
        }
    }

    ImageBuffer::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("high-depth u16 Reduce shape mismatch".into()))
}

/// Average f32 channels independently while preserving native f32 storage.
pub(super) fn reduce_f32_pixels<P>(
    image: &ImageBuffer<P, Vec<f32>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<ImageBuffer<P, Vec<f32>>, PilError>
where
    P: Pixel<Subpixel = f32>,
{
    let (width, height) = image.dimensions();
    let (x_factor, y_factor) = (x_factor.max(1), y_factor.max(1));
    let channels = P::CHANNEL_COUNT as usize;
    let (output_width, output_height, output_samples) =
        output_shape(width, height, channels, x_factor, y_factor)?;
    let source = image.as_raw();
    let mut output = Vec::with_capacity(output_samples);

    for output_y in 0..output_height {
        let source_y = output_y * y_factor;
        let block_height = y_factor.min(height - source_y);
        for output_x in 0..output_width {
            let source_x = output_x * x_factor;
            let block_width = x_factor.min(width - source_x);
            let count = u64::from(block_width) * u64::from(block_height);
            for channel in 0..channels {
                let mut sum = 0.0f64;
                for row in 0..block_height as usize {
                    let row_start = (source_y as usize + row) * width as usize;
                    for column in 0..block_width as usize {
                        let pixel = row_start + source_x as usize + column;
                        sum += f64::from(source[pixel * channels + channel]);
                    }
                }
                output.push((sum / count as f64) as f32);
            }
        }
    }

    ImageBuffer::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("high-depth f32 Reduce shape mismatch".into()))
}
