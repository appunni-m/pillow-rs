//! Scalar I/F affine interpolation path.

use crate::pipeline::ResampleFilter;

use super::cubic::{
    rotate_cubic_horizontal_f32, rotate_cubic_horizontal_i32, rotate_cubic_vertical_f64,
    rotate_cubic_vertical_f64_fma,
};

/// Apply Pillow's bilinear/bicubic transform to native four-byte `I`/`F`
/// samples.
///
/// These modes are represented by four bytes in the backend image, but the
/// transform kernel operates on one signed 32-bit or float32 sample. Pillow's
/// scalar affine kernels accumulate the bilinear value in double precision;
/// `F` then stores float32 and `I` truncates the result toward zero.
pub(crate) fn rotate_arbitrary_scalar(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    channels: usize,
    affine: [f64; 6],
    fill: (u8, u8, u8, u8),
    filter: ResampleFilter,
    is_float: bool,
    output: &mut [u8],
) {
    let [a, b, c, d, e, f] = affine;
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let scalar_fill = if is_float {
        f32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    } else {
        i32::from_le_bytes([fill.0, fill.1, fill.2, fill.3]) as f64
    };

    if source_width == 0 || source_height == 0 {
        for output_index in (0..destination_width as usize * destination_height as usize)
            .map(|index| index * channels)
        {
            let bytes = if is_float {
                (scalar_fill as f32).to_le_bytes()
            } else {
                (scalar_fill as i32).to_le_bytes()
            };
            output[output_index..output_index + 4].copy_from_slice(&bytes);
        }
        return;
    }

    for dy in 0..destination_height {
        for dx in 0..destination_width {
            let xin = dx as f64 + 0.5;
            let yin = dy as f64 + 0.5;
            // Geometry.c evaluates affine coordinates as a product followed
            // by a fused multiply-add and a separate translation add for both
            // typed 32-bit and FLOAT32 images. The filter subtracts 0.5 only
            // after the center-space bounds check.
            let sx = a.mul_add(xin, b * yin) + c;
            let sy = d.mul_add(xin, e * yin) + f;
            let output_index = (dy * destination_width + dx) as usize * channels;

            let value = if sx >= 0.0
                && sx < source_width as f64
                && sy >= 0.0
                && sy < source_height as f64
            {
                let sample_x = sx - 0.5;
                let sample_y = sy - 0.5;
                let read = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    if is_float {
                        f32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    } else {
                        i32::from_le_bytes([
                            source[index],
                            source[index + 1],
                            source[index + 2],
                            source[index + 3],
                        ]) as f64
                    }
                };
                let read_f32 = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    f32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ])
                };
                let read_i32 = |x: u32, y: u32| {
                    let index = ((y * source_width + x) as usize) * channels;
                    i32::from_le_bytes([
                        source[index],
                        source[index + 1],
                        source[index + 2],
                        source[index + 3],
                    ])
                };
                if matches!(filter, ResampleFilter::Bicubic) {
                    let floor_x = sample_x.floor() as i64;
                    let floor_y = sample_y.floor() as i64;
                    let base_x = floor_x - 1;
                    let base_y = floor_y - 1;
                    let fx = sample_x - floor_x as f64;
                    let fy = sample_y - floor_y as f64;
                    let mut rows = [0.0f64; 4];
                    for (row, output) in rows.iter_mut().enumerate() {
                        let yy =
                            (base_y + row as i64).clamp(0, i64::from(source_height - 1)) as u32;
                        let taps = [
                            (base_x).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 1).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 2).clamp(0, i64::from(source_width - 1)) as u32,
                            (base_x + 3).clamp(0, i64::from(source_width - 1)) as u32,
                        ];
                        *output = if is_float {
                            rotate_cubic_horizontal_f32(
                                [
                                    read_f32(taps[0], yy),
                                    read_f32(taps[1], yy),
                                    read_f32(taps[2], yy),
                                    read_f32(taps[3], yy),
                                ],
                                fx,
                            )
                        } else {
                            rotate_cubic_horizontal_i32(
                                [
                                    read_i32(taps[0], yy),
                                    read_i32(taps[1], yy),
                                    read_i32(taps[2], yy),
                                    read_i32(taps[3], yy),
                                ],
                                fx,
                            )
                        };
                    }
                    if is_float {
                        rotate_cubic_vertical_f64_fma(rows, fy)
                    } else {
                        rotate_cubic_vertical_f64(rows, fy)
                    }
                } else {
                    let floor_x = sample_x.floor() as i64;
                    let floor_y = sample_y.floor() as i64;
                    let x0 = floor_x.clamp(0, i64::from(source_width - 1)) as u32;
                    let y0 = floor_y.clamp(0, i64::from(source_height - 1)) as u32;
                    let x1 = (floor_x + 1).clamp(0, i64::from(source_width - 1)) as u32;
                    let y1 = (floor_y + 1).clamp(0, i64::from(source_height - 1)) as u32;
                    let fx = sample_x - floor_x as f64;
                    let fy = sample_y - floor_y as f64;
                    let p00 = read(x0, y0);
                    let p10 = read(x1, y0);
                    let p01 = read(x0, y1);
                    let p11 = read(x1, y1);
                    let horizontal_top = if is_float {
                        f64::from((p10 as f32) - (p00 as f32)).mul_add(fx, p00)
                    } else {
                        let p00 = p00 as i32;
                        let p10 = p10 as i32;
                        f64::from(p10.wrapping_sub(p00)) * fx + f64::from(p00)
                    };
                    let horizontal_bottom = if is_float {
                        f64::from((p11 as f32) - (p01 as f32)).mul_add(fx, p01)
                    } else {
                        let p01 = p01 as i32;
                        let p11 = p11 as i32;
                        f64::from(p11.wrapping_sub(p01)) * fx + f64::from(p01)
                    };
                    if is_float {
                        (horizontal_bottom - horizontal_top).mul_add(fy, horizontal_top)
                    } else {
                        (horizontal_bottom - horizontal_top) * fy + horizontal_top
                    }
                }
            } else {
                scalar_fill
            };

            let bytes = if is_float {
                (value as f32).to_le_bytes()
            } else {
                (value as i32).to_le_bytes()
            };
            output[output_index..output_index + 4].copy_from_slice(&bytes);
        }
    }
}
