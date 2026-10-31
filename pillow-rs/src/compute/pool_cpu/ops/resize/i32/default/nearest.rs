//! Direct scalar-word sampling for Pillow's ImageI32 Resize mode.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(
    source: &[i32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
) -> Result<RgbaImage, PilError> {
    let (width, height) = destination_size;
    let dims = CheckedDims::new(width, height, 4)?;
    let mut output = vec![0_i32; dims.total_pixels()];
    ordinary_samples(source, source_size, destination_size, &mut output);
    let bytes = encode(&output, dims.total_bytes());
    RgbaImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}

pub(crate) fn execute_boxed(
    input: &RgbaImage,
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> Result<RgbaImage, PilError> {
    let (destination_width, destination_height) = destination_size;
    let (source_width, source_height) = input.dimensions();
    if source_width == 0
        || source_height == 0
        || destination_width == 0
        || destination_height == 0
    {
        let output_dims = CheckedDims::new_allow_empty(destination_width, destination_height, 4)?;
        let output = output_dims.alloc_buffer();
        return RgbaImage::from_raw(destination_width, destination_height, output).ok_or_else(|| {
            PilError::ValueError("resize_i: failed to create output buffer".into())
        });
    }

    let (left, top, right, bottom) = bounds;
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    if destination_size == input.dimensions()
        && left.is_finite()
        && top.is_finite()
        && right.is_finite()
        && bottom.is_finite()
        && left == 0.0
        && top == 0.0
        && right == source_width as f32
        && bottom == source_height as f32
    {
        return Ok(input.clone());
    }

    let output_dims = CheckedDims::new_allow_empty(destination_width, destination_height, 4)?;
    let mut output = output_dims.alloc_buffer();
    let scale_x = f64::from(right - left) / f64::from(destination_width);
    let scale_y = f64::from(bottom - top) / f64::from(destination_height);
    let source = input.as_raw();
    let output_stride = output_dims.row_stride();
    let source_stride = source_width as usize * 4;
    // Pillow narrows the box to f32, promotes it back to f64, and advances
    // affine coordinates cumulatively. Recomputing each pixel from its index
    // can change exact nearest boundaries.
    let mut source_y = f64::from(top) + scale_y * 0.5;
    for destination_y in 0..destination_height as usize {
        let source_y_index = source_y.floor().clamp(0.0, f64::from(source_height - 1)) as usize;
        let source_row = source_y_index * source_stride;
        let destination_row = destination_y * output_stride;
        let mut source_x = f64::from(left) + scale_x * 0.5;
        for destination_x in 0..destination_width as usize {
            let source_x_index = source_x.floor().clamp(0.0, f64::from(source_width - 1)) as usize;
            let source_start = source_row + source_x_index * 4;
            let destination_start = destination_row + destination_x * 4;
            output[destination_start..destination_start + 4]
                .copy_from_slice(&source[source_start..source_start + 4]);
            source_x += scale_x;
        }
        source_y += scale_y;
    }

    RgbaImage::from_raw(destination_width, destination_height, output)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}

fn ordinary_samples(
    source: &[i32],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    output: &mut [i32],
) {
    let (source_width, source_height) = source_size;
    let (width, height) = destination_size;
    for y in 0..height {
        let source_y = (((f64::from(y) + 0.5) * f64::from(source_height)
            / f64::from(height))
        .floor() as i64)
            .clamp(0, i64::from(source_height) - 1) as u32;
        for x in 0..width {
            let source_x = (((f64::from(x) + 0.5) * f64::from(source_width)
                / f64::from(width))
            .floor() as i64)
                .clamp(0, i64::from(source_width) - 1) as u32;
            output[(y * width + x) as usize] = source[(source_y * source_width + source_x) as usize];
        }
    }
}

fn encode(samples: &[i32], byte_count: usize) -> Vec<u8> {
    let mut bytes = vec![0_u8; byte_count];
    for (sample, output) in samples.iter().zip(bytes.chunks_exact_mut(4)) {
        output.copy_from_slice(&sample.to_le_bytes());
    }
    bytes
}
