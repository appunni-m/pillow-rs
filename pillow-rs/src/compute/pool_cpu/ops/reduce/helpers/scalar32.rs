//! Native scalar reducers for the Pillow `I` and `F` byte carriers.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::round_up;
use crate::raster::RgbaImage;

fn scalar_index(x: u32, y: u32, width: u32) -> usize {
    (y as usize * width as usize + x as usize) * 4
}

fn read_i32(source: &[u8], index: usize) -> i32 {
    i32::from_le_bytes(source[index..index + 4].try_into().expect("scalar sample has four bytes"))
}

fn read_f32(source: &[u8], index: usize) -> f32 {
    f32::from_le_bytes(source[index..index + 4].try_into().expect("scalar sample has four bytes"))
}

fn output_bytes(width: u32, height: u32) -> Result<Vec<u8>, PilError> {
    CheckedDims::new(width, height, 4).map(CheckedDims::alloc_buffer)
}

fn reduce_i32_2x2(
    source: &[u8],
    source_width: u32,
    output_width: u32,
    output_height: u32,
) -> Result<Vec<u8>, PilError> {
    let mut output = output_bytes(output_width, output_height)?;
    for y in 0..output_height as usize {
        let top = y * 2 * source_width as usize * 4;
        let bottom = top + source_width as usize * 4;
        for x in 0..output_width as usize {
            let first = top + x * 8;
            let second = bottom + x * 8;
            let quartet = read_i32(source, first)
                .wrapping_add(read_i32(source, first + 4))
                .wrapping_add(read_i32(source, second))
                .wrapping_add(read_i32(source, second + 4));
            let value = round_up(f64::from(quartet) / 4.0) as i32;
            let destination = (y * output_width as usize + x) * 4;
            output[destination..destination + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(output)
}

fn reduce_i32_interior_sum(
    source: &[u8],
    width: u32,
    source_x: u32,
    source_y: u32,
    block_width: u32,
    block_height: u32,
) -> f64 {
    let mut sum = 0.0f64;
    let mut dy = 0;
    while dy + 1 < block_height {
        let mut dx = 0;
        while dx + 1 < block_width {
            let top = read_i32(source, scalar_index(source_x + dx, source_y + dy, width));
            let top_right = read_i32(source, scalar_index(source_x + dx + 1, source_y + dy, width));
            let bottom = read_i32(source, scalar_index(source_x + dx, source_y + dy + 1, width));
            let bottom_right =
                read_i32(source, scalar_index(source_x + dx + 1, source_y + dy + 1, width));
            sum += f64::from(top.wrapping_add(top_right).wrapping_add(bottom).wrapping_add(bottom_right));
            dx += 2;
        }
        if dx < block_width {
            let first = read_i32(source, scalar_index(source_x + dx, source_y + dy, width));
            let second = read_i32(source, scalar_index(source_x + dx, source_y + dy + 1, width));
            sum += f64::from(first.wrapping_add(second));
        }
        dy += 2;
    }
    if dy < block_height {
        for dx in (0..block_width).step_by(2) {
            let first = read_i32(source, scalar_index(source_x + dx, source_y + dy, width));
            if dx + 1 < block_width {
                let second = read_i32(source, scalar_index(source_x + dx + 1, source_y + dy, width));
                sum += f64::from(first.wrapping_add(second));
            } else {
                sum += f64::from(first);
            }
        }
    }
    sum
}

fn reduce_i32_edge_sum(
    source: &[u8],
    width: u32,
    source_x: u32,
    source_y: u32,
    block_width: u32,
    block_height: u32,
) -> f64 {
    let mut sum = 0.0f64;
    for dy in 0..block_height {
        for dx in 0..block_width {
            sum += f64::from(read_i32(
                source,
                scalar_index(source_x + dx, source_y + dy, width),
            ));
        }
    }
    sum
}

fn reduce_i32_general(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let mut output = output_bytes(output_width, output_height)?;
    let main_width = source_width / x_factor;
    let main_height = source_height / y_factor;
    for y in 0..output_height {
        let source_y = y * y_factor;
        let block_height = y_factor.min(source_height - source_y);
        for x in 0..output_width {
            let source_x = x * x_factor;
            let block_width = x_factor.min(source_width - source_x);
            let sum = if x < main_width && y < main_height {
                reduce_i32_interior_sum(source, source_width, source_x, source_y, block_width, block_height)
            } else {
                reduce_i32_edge_sum(source, source_width, source_x, source_y, block_width, block_height)
            };
            let count = f64::from(block_width) * f64::from(block_height);
            let value = round_up(sum / count) as i32;
            let destination = (y as usize * output_width as usize + x as usize) * 4;
            output[destination..destination + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(output)
}

/// Reduces the signed scalar samples stored in the I-mode four-byte carrier.
pub(crate) fn reduce_i32(
    image: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<RgbaImage, PilError> {
    let width = image.width();
    let height = image.height();
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let source = image.as_raw();
    let output = if x_factor == 2 && y_factor == 2 && width % 2 == 0 && height % 2 == 0 {
        reduce_i32_2x2(source, width, output_width, output_height)?
    } else {
        reduce_i32_general(source, width, height, output_width, output_height, x_factor, y_factor)?
    };
    RgbaImage::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("failed to construct I Reduce output".into()))
}

fn reduce_f32_2x2(
    source: &[u8],
    source_width: u32,
    output_width: u32,
    output_height: u32,
) -> Result<Vec<u8>, PilError> {
    let mut output = output_bytes(output_width, output_height)?;
    for y in 0..output_height as usize {
        let top = y * 2 * source_width as usize;
        let bottom = top + source_width as usize;
        for x in 0..output_width as usize {
            let top_left = read_f32(source, (top + x * 2) * 4);
            let top_right = read_f32(source, (top + x * 2 + 1) * 4);
            let bottom_left = read_f32(source, (bottom + x * 2) * 4);
            let bottom_right = read_f32(source, (bottom + x * 2 + 1) * 4);
            let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
            let value = (f64::from(quartet) * 0.25) as f32;
            let destination = (y * output_width as usize + x) * 4;
            output[destination..destination + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(output)
}

fn reduce_f32_interior_sum(
    source: &[u8],
    width: u32,
    source_x: u32,
    source_y: u32,
    block_width: u32,
    block_height: u32,
) -> f64 {
    let mut sum = 0.0f64;
    let mut dy = 0;
    while dy + 1 < block_height {
        let mut dx = 0;
        while dx + 1 < block_width {
            let top = read_f32(source, scalar_index(source_x + dx, source_y + dy, width));
            let top_right = read_f32(source, scalar_index(source_x + dx + 1, source_y + dy, width));
            let bottom = read_f32(source, scalar_index(source_x + dx, source_y + dy + 1, width));
            let bottom_right =
                read_f32(source, scalar_index(source_x + dx + 1, source_y + dy + 1, width));
            let quartet = ((top + top_right) + bottom) + bottom_right;
            sum += f64::from(quartet);
            dx += 2;
        }
        if dx < block_width {
            let first = read_f32(source, scalar_index(source_x + dx, source_y + dy, width));
            let second = read_f32(source, scalar_index(source_x + dx, source_y + dy + 1, width));
            sum += f64::from(first + second);
        }
        dy += 2;
    }
    if dy < block_height {
        for dx in (0..block_width).step_by(2) {
            let first = read_f32(source, scalar_index(source_x + dx, source_y + dy, width));
            if dx + 1 < block_width {
                let second = read_f32(source, scalar_index(source_x + dx + 1, source_y + dy, width));
                sum += f64::from(first + second);
            } else {
                sum += f64::from(first);
            }
        }
    }
    sum
}

fn reduce_f32_edge_sum(
    source: &[u8],
    width: u32,
    source_x: u32,
    source_y: u32,
    block_width: u32,
    block_height: u32,
) -> f64 {
    let mut sum = 0.0f64;
    for dy in 0..block_height {
        for dx in 0..block_width {
            sum += f64::from(read_f32(
                source,
                scalar_index(source_x + dx, source_y + dy, width),
            ));
        }
    }
    sum
}

fn reduce_f32_general(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let mut output = output_bytes(output_width, output_height)?;
    let main_width = source_width / x_factor;
    let main_height = source_height / y_factor;
    for y in 0..output_height {
        let source_y = y * y_factor;
        let block_height = y_factor.min(source_height - source_y);
        for x in 0..output_width {
            let source_x = x * x_factor;
            let block_width = x_factor.min(source_width - source_x);
            let sum = if x < main_width && y < main_height {
                reduce_f32_interior_sum(source, source_width, source_x, source_y, block_width, block_height)
            } else {
                reduce_f32_edge_sum(source, source_width, source_x, source_y, block_width, block_height)
            };
            let count = f64::from(block_width) * f64::from(block_height);
            let value = (sum * (1.0 / count)) as f32;
            let destination = (y as usize * output_width as usize + x as usize) * 4;
            output[destination..destination + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(output)
}

/// Reduces the float scalar samples stored in the F-mode four-byte carrier.
pub(crate) fn reduce_f32(image: &RgbaImage, x_factor: u32, y_factor: u32) -> Result<RgbaImage, PilError> {
    let width = image.width();
    let height = image.height();
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let source = image.as_raw();
    let output = if x_factor == 2 && y_factor == 2 && width % 2 == 0 && height % 2 == 0 {
        reduce_f32_2x2(source, width, output_width, output_height)?
    } else {
        reduce_f32_general(source, width, height, output_width, output_height, x_factor, y_factor)?
    };
    RgbaImage::from_raw(output_width, output_height, output)
        .ok_or_else(|| PilError::InternalError("failed to construct F Reduce output".into()))
}
