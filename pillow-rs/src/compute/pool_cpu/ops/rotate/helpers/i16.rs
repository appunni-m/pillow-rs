//! Native u16 rotation, including Pillow's byte-plane I;16 filter behavior.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::{discrete_angle, rotate_geometry};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

pub(crate) fn execute<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    request: RotateRequest,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    if is_identity(request) {
        return Ok(image.clone());
    }
    if let Some(degree) = discrete_angle(request) {
        let (width, height) = image.dimensions();
        if degree == 180 || request.expand || width == height {
            return rotate_discrete(image, degree);
        }
    }
    if request.nearest {
        return execute_nearest::<BIG_ENDIAN>(image, request);
    }
    execute_filtered::<BIG_ENDIAN>(image, request)
}

fn is_identity(request: RotateRequest) -> bool {
    request.center.is_none()
        && request.translate.is_none()
        && request.angle.rem_euclid(360.0).abs() <= f64::EPSILON
}

fn rotate_discrete(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    degree: u32,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let (width, height) = image.dimensions();
    let (out_width, out_height) = if degree == 180 { (width, height) } else { (height, width) };
    let dimensions = CheckedDims::new(out_width, out_height, 1)?;
    let mut output = vec![0; dimensions.total_pixels()];
    for y in 0..out_height {
        for x in 0..out_width {
            let (sx, sy) = match degree {
                90 => (width - 1 - y, x),
                180 => (width - 1 - x, height - 1 - y),
                270 => (y, height - 1 - x),
                _ => unreachable!("discrete Rotate degree was validated"),
            };
            output[(y * out_width + x) as usize] = image.as_raw()[(sy * width + sx) as usize];
        }
    }
    ImageBuffer::from_raw(out_width, out_height, output)
        .ok_or_else(|| PilError::InternalError("rotate I;16 result shape mismatch".into()))
}

fn execute_nearest<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    request: RotateRequest,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, 1)?;
    let fill = request.fill.map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
    let (width, height) = image.dimensions();
    let [a, b, c, d, e, f] = geometry.affine;
    let mut output = vec![fill; dimensions.total_pixels()];
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            // Pillow's I;16 nearest branch uses Geometry.c's floating-point
            // affine mapping; other byte carriers use the 16.16 sampler.
            let destination_x = f64::from(x) + 0.5;
            let destination_y = f64::from(y) + 0.5;
            let source_x = a.mul_add(destination_x, b * destination_y) + c;
            let source_y = d.mul_add(destination_x, e * destination_y) + f;
            if source_x >= 0.0
                && source_x < f64::from(width)
                && source_y >= 0.0
                && source_y < f64::from(height)
            {
                output[(y * geometry.width + x) as usize] = image.as_raw()
                    [(source_y as u32 * width + source_x as u32) as usize];
            }
        }
    }
    ImageBuffer::from_raw(geometry.width, geometry.height, output)
        .ok_or_else(|| PilError::InternalError("rotate I;16 result shape mismatch".into()))
}

fn execute_filtered<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    request: RotateRequest,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, 1)?;
    let fill_value = request.fill.map_or(0, |fill| u16::from_le_bytes([fill.0, fill.1]));
    let fill_bytes = encode::<BIG_ENDIAN>(fill_value);
    let mut output = vec![fill_value; dimensions.total_pixels()];
    let [a, b, c, d, e, f] = geometry.affine;
    let (source_width, source_height) = image.dimensions();
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            let sx = a.mul_add(f64::from(x) + 0.5, b * (f64::from(y) + 0.5)) + c - 0.5;
            let sy = d.mul_add(f64::from(x) + 0.5, e * (f64::from(y) + 0.5)) + f - 0.5;
            let index = (y * geometry.width + x) as usize;
            if !in_support(sx, sy, source_width, source_height) {
                continue;
            }
            let value = if matches!(request.filter, ResampleFilter::Bicubic) {
                sample_bicubic::<BIG_ENDIAN>(image, sx, sy)
            } else {
                sample_bilinear::<BIG_ENDIAN>(image, sx, sy)
            };
            output[index] = decode::<BIG_ENDIAN>([value as u8, fill_bytes[1]]);
        }
    }
    ImageBuffer::from_raw(geometry.width, geometry.height, output)
        .ok_or_else(|| PilError::InternalError("rotate I;16 result shape mismatch".into()))
}

fn sample_bilinear<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x: f64,
    y: f64,
) -> f64 {
    let (width, height) = image.dimensions();
    let x = x.clamp(0.0, f64::from(width - 1));
    let y = y.clamp(0.0, f64::from(height - 1));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - f64::from(x0), y - f64::from(y0));
    let (p00, p10, p01, p11) = (
        plane_sample::<BIG_ENDIAN>(image, x0, y0) as f64,
        plane_sample::<BIG_ENDIAN>(image, x1, y0) as f64,
        plane_sample::<BIG_ENDIAN>(image, x0, y1) as f64,
        plane_sample::<BIG_ENDIAN>(image, x1, y1) as f64,
    );
    let top = (p10 - p00).mul_add(fx, p00);
    let bottom = (p11 - p01).mul_add(fx, p01);
    (bottom - top).mul_add(fy, top)
}

fn sample_bicubic<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x: f64,
    y: f64,
) -> f64 {
    let (width, height) = image.dimensions();
    let (floor_x, floor_y) = (x.floor() as i64, y.floor() as i64);
    let (base_x, base_y) = (floor_x - 1, floor_y - 1);
    let (fx, fy) = (x - floor_x as f64, y - floor_y as f64);
    let mut rows = [0.0; 4];
    for (row, value) in rows.iter_mut().enumerate() {
        let source_y = (base_y + row as i64).clamp(0, i64::from(height - 1)) as u32;
        let samples = [0, 1, 2, 3].map(|tap| {
            let source_x = (base_x + tap).clamp(0, i64::from(width - 1)) as u32;
            f64::from(plane_sample::<BIG_ENDIAN>(image, source_x, source_y))
        });
        *value = super::cubic::horizontal_f64(samples, fx);
    }
    super::cubic::vertical_f64(rows, fy)
}

fn plane_sample<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    x: u32,
    y: u32,
) -> u8 {
    let width = image.width();
    let sample = image.as_raw()[(y * width + x / 2) as usize];
    encode::<BIG_ENDIAN>(sample)[(x % 2) as usize]
}

fn in_support(x: f64, y: f64, width: u32, height: u32) -> bool {
    x >= -0.5 && x < f64::from(width) - 0.5 && y >= -0.5 && y < f64::from(height) - 0.5
}

fn encode<const BIG_ENDIAN: bool>(value: u16) -> [u8; 2] {
    if BIG_ENDIAN { value.to_be_bytes() } else { value.to_le_bytes() }
}

fn decode<const BIG_ENDIAN: bool>(bytes: [u8; 2]) -> u16 {
    if BIG_ENDIAN { u16::from_be_bytes(bytes) } else { u16::from_le_bytes(bytes) }
}
