//! Native unsigned 16-bit luma reduction for Pillow's `I;16*` modes.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Luma};

pub(super) fn reduce_luma16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x_factor: u32,
    y_factor: u32,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let (width, height) = image.dimensions();
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let output_len = usize::try_from(output_width)
        .ok()
        .and_then(|width| {
            usize::try_from(output_height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| PilError::InternalError("I;16 reduce output size overflow".into()))?;
    let source = image.as_raw();
    let source_width = width as usize;
    let mut output = Vec::with_capacity(output_len);

    for output_y in 0..output_height {
        let source_y = output_y * y_factor;
        let block_height = y_factor.min(height - source_y);
        for output_x in 0..output_width {
            let source_x = output_x * x_factor;
            let block_width = x_factor.min(width - source_x);
            let sample_count = u64::from(block_width) * u64::from(block_height);
            let mut sum = 0u64;
            for row in 0..block_height as usize {
                let start = (source_y as usize + row) * source_width + source_x as usize;
                let end = start + block_width as usize;
                for &sample in &source[start..end] {
                    sum += u64::from(sample);
                }
            }
            output.push(((sum + sample_count / 2) / sample_count) as u16);
        }
    }

    ImageBuffer::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("I;16 reduce output shape mismatch".into()))
}
