//! Native u16 and f32 sample rotation for exact core-only mode strategies.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::{
    discrete_angle, rotate_geometry,
};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

pub(crate) trait NativeSample: Copy + Default {
    fn to_f64(self) -> f64;
    fn from_f64(value: f64) -> Self;
    fn fill_sample(value: u8) -> Self;
    fn alpha_max() -> f64;
}

impl NativeSample for u16 {
    fn to_f64(self) -> f64 { f64::from(self) }
    fn from_f64(value: f64) -> Self { value.clamp(0.0, f64::from(u16::MAX)) as u16 }
    fn fill_sample(value: u8) -> Self { u16::from(value) * 257 }
    fn alpha_max() -> f64 { f64::from(u16::MAX) }
}

impl NativeSample for f32 {
    fn to_f64(self) -> f64 { f64::from(self) }
    fn from_f64(value: f64) -> Self { value as f32 }
    fn fill_sample(value: u8) -> Self { f32::from(value) / 255.0 }
    fn alpha_max() -> f64 { 1.0 }
}

pub(crate) fn execute<P, S, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<S>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<S>>, PilError>
where
    P: Pixel<Subpixel = S>,
    S: NativeSample,
{
    if is_identity(request) {
        return Ok(image.clone());
    }
    if let Some(degree) = discrete_angle(request) {
        let (width, height) = image.dimensions();
        if degree == 180 || request.expand || width == height {
            return rotate_discrete::<P, S, CHANNELS>(image, degree);
        }
        return rotate_90_non_expand::<P, S, CHANNELS>(image, degree, request.fill);
    }
    execute_affine::<P, S, CHANNELS, FORCE_NEAREST, ALPHA>(image, request)
}

fn is_identity(request: RotateRequest) -> bool {
    request.center.is_none()
        && request.translate.is_none()
        && request.angle.rem_euclid(360.0).abs() <= f64::EPSILON
}

fn execute_affine<P, S, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<S>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<S>>, PilError>
where
    P: Pixel<Subpixel = S>,
    S: NativeSample,
{
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, CHANNELS as u8)?;
    let mut output = vec![S::default(); dimensions.total_pixels() * CHANNELS];
    let fill = native_fill::<S, CHANNELS>(request.fill);
    let nearest = FORCE_NEAREST || request.nearest;
    if nearest {
        nearest_affine::<S, CHANNELS>(image.as_raw(), image.dimensions(), geometry, fill, &mut output);
    } else {
        let mut source = image.as_raw().clone();
        if ALPHA { premultiply::<S, CHANNELS>(&mut source); }
        filtered_affine::<S, CHANNELS>(&source, image.dimensions(), geometry, fill, request.filter, &mut output);
        if ALPHA { unpremultiply::<S, CHANNELS>(&mut output); }
    }
    ImageBuffer::from_raw(geometry.width, geometry.height, output)
        .ok_or_else(|| PilError::InternalError("Rotate native sample result shape mismatch".into()))
}

fn rotate_discrete<P, S, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<S>>,
    degree: u32,
) -> Result<ImageBuffer<P, Vec<S>>, PilError>
where
    P: Pixel<Subpixel = S>,
    S: NativeSample,
{
    let (width, height) = image.dimensions();
    let (out_width, out_height) = if degree == 180 { (width, height) } else { (height, width) };
    let dimensions = CheckedDims::new(out_width, out_height, CHANNELS as u8)?;
    let mut output = vec![S::default(); dimensions.total_pixels() * CHANNELS];
    for y in 0..out_height {
        for x in 0..out_width {
            let (source_x, source_y) = match degree {
                90 => (width - 1 - y, x),
                180 => (width - 1 - x, height - 1 - y),
                270 => (y, height - 1 - x),
                _ => unreachable!("discrete Rotate degree was validated"),
            };
            copy_pixel::<S, CHANNELS>(image.as_raw(), &mut output, width, out_width, source_x, source_y, x, y);
        }
    }
    ImageBuffer::from_raw(out_width, out_height, output)
        .ok_or_else(|| PilError::InternalError("Rotate native sample result shape mismatch".into()))
}

fn rotate_90_non_expand<P, S, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<S>>,
    degree: u32,
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<ImageBuffer<P, Vec<S>>, PilError>
where
    P: Pixel<Subpixel = S>,
    S: NativeSample,
{
    let (width, height) = image.dimensions();
    let dimensions = CheckedDims::new(width, height, CHANNELS as u8)?;
    let mut output = vec![S::default(); dimensions.total_pixels() * CHANNELS];
    let fill = native_fill::<S, CHANNELS>(fill);
    output.chunks_exact_mut(CHANNELS).for_each(|pixel| pixel.copy_from_slice(&fill));
    let gap_x = width as f64 - height as f64;
    let gap_y = height as f64 - width as f64;
    let (dx, dy) = if degree == 90 {
        ((gap_x / 2.0).floor() as i32, (gap_y / 2.0).ceil() as i32)
    } else {
        ((gap_x / 2.0).ceil() as i32, (gap_y / 2.0).floor() as i32)
    };
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = if degree == 270 {
                (y as i32 - dy, height as i32 - 1 - x as i32 + dx)
            } else {
                (width as i32 - 1 - y as i32 + dy, x as i32 - dx)
            };
            if sx >= 0 && sx < width as i32 && sy >= 0 && sy < height as i32 {
                copy_pixel::<S, CHANNELS>(image.as_raw(), &mut output, width, width, sx as u32, sy as u32, x, y);
            }
        }
    }
    ImageBuffer::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("Rotate native sample result shape mismatch".into()))
}

fn copy_pixel<S: Copy, const CHANNELS: usize>(
    source: &[S], output: &mut [S], source_width: u32, output_width: u32,
    sx: u32, sy: u32, dx: u32, dy: u32,
) {
    let source_start = (sy * source_width + sx) as usize * CHANNELS;
    let output_start = (dy * output_width + dx) as usize * CHANNELS;
    output[output_start..output_start + CHANNELS]
        .copy_from_slice(&source[source_start..source_start + CHANNELS]);
}

fn native_fill<S: NativeSample, const CHANNELS: usize>(fill: Option<(u8, u8, u8, u8)>) -> [S; CHANNELS] {
    let (r, g, b, a) = fill.unwrap_or((0, 0, 0, 0));
    std::array::from_fn(|channel| {
        let value = if CHANNELS == 2 && channel == 1 { a } else {
            match channel { 0 => r, 1 => g, 2 => b, _ => a }
        };
        S::fill_sample(value)
    })
}

fn nearest_affine<S: Copy, const CHANNELS: usize>(
    source: &[S], source_size: (u32, u32), geometry: super::geometry::Geometry,
    fill: [S; CHANNELS], output: &mut [S],
) {
    let [a, b, c, d, e, f] = geometry.affine;
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let (xx, yx, xy, yy) = (fixed(a), fixed(b), fixed(d), fixed(e));
    let (ox, oy) = (fixed(c + a * 0.5 + b * 0.5), fixed(f + d * 0.5 + e * 0.5));
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            let sx = ox + i64::from(y) * yx + i64::from(x) * xx;
            let sy = oy + i64::from(y) * yy + i64::from(x) * xy;
            let (sx, sy) = (sx >> 16, sy >> 16);
            let start = (y * geometry.width + x) as usize * CHANNELS;
            if sx >= 0 && sx < i64::from(source_size.0) && sy >= 0 && sy < i64::from(source_size.1) {
                let source_start = (sy as u32 * source_size.0 + sx as u32) as usize * CHANNELS;
                output[start..start + CHANNELS].copy_from_slice(&source[source_start..source_start + CHANNELS]);
            } else {
                output[start..start + CHANNELS].copy_from_slice(&fill);
            }
        }
    }
}

fn filtered_affine<S: NativeSample, const CHANNELS: usize>(
    source: &[S], size: (u32, u32), geometry: super::geometry::Geometry,
    fill: [S; CHANNELS], filter: ResampleFilter, output: &mut [S],
) {
    let [a, b, c, d, e, f] = geometry.affine;
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            let dx = f64::from(x) + 0.5;
            let dy = f64::from(y) + 0.5;
            let sx = a.mul_add(dx, b * dy) + c - 0.5;
            let sy = d.mul_add(dx, e * dy) + f - 0.5;
            let start = (y * geometry.width + x) as usize * CHANNELS;
            if !in_filter_support(sx, sy, size) {
                output[start..start + CHANNELS].copy_from_slice(&fill);
            } else {
                sample_pixel::<S, CHANNELS>(source, size, sx, sy, filter, &mut output[start..start + CHANNELS]);
            }
        }
    }
}

fn sample_pixel<S: NativeSample, const CHANNELS: usize>(
    source: &[S], size: (u32, u32), x: f64, y: f64, filter: ResampleFilter, output: &mut [S],
) {
    let (width, height) = size;
    for (channel, value) in output.iter_mut().enumerate() {
        let sample = if matches!(filter, ResampleFilter::Bicubic) {
            bicubic::<S, CHANNELS>(source, width, height, x, y, channel)
        } else {
            bilinear::<S, CHANNELS>(source, width, height, x, y, channel)
        };
        *value = S::from_f64(sample);
    }
}

fn bilinear<S: NativeSample, const CHANNELS: usize>(
    source: &[S], width: u32, height: u32, x: f64, y: f64, channel: usize,
) -> f64 {
    let (x, y) = (x.clamp(0.0, f64::from(width - 1)), y.clamp(0.0, f64::from(height - 1)));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - f64::from(x0), y - f64::from(y0));
    let read = |px, py| source[(py * width + px) as usize * CHANNELS + channel].to_f64();
    let (p00, p10, p01, p11) = (read(x0, y0), read(x1, y0), read(x0, y1), read(x1, y1));
    let top = (p10 - p00).mul_add(fx, p00);
    let bottom = (p11 - p01).mul_add(fx, p01);
    (bottom - top).mul_add(fy, top)
}

fn bicubic<S: NativeSample, const CHANNELS: usize>(
    source: &[S], width: u32, height: u32, x: f64, y: f64, channel: usize,
) -> f64 {
    let (floor_x, floor_y) = (x.floor() as i64, y.floor() as i64);
    let (base_x, base_y) = (floor_x - 1, floor_y - 1);
    let (fx, fy) = (x - floor_x as f64, y - floor_y as f64);
    let mut rows = [0.0; 4];
    for (row, value) in rows.iter_mut().enumerate() {
        let sy = (base_y + row as i64).clamp(0, i64::from(height - 1)) as u32;
        let taps = [0, 1, 2, 3].map(|tap| {
            let sx = (base_x + tap).clamp(0, i64::from(width - 1)) as u32;
            source[(sy * width + sx) as usize * CHANNELS + channel].to_f64()
        });
        *value = super::cubic::horizontal_f64(taps, fx);
    }
    super::cubic::vertical_f64(rows, fy)
}

fn in_filter_support(x: f64, y: f64, size: (u32, u32)) -> bool {
    x >= -0.5 && x < f64::from(size.0) - 0.5 && y >= -0.5 && y < f64::from(size.1) - 0.5
}

fn premultiply<S: NativeSample, const CHANNELS: usize>(samples: &mut [S]) {
    let scale = S::alpha_max();
    for pixel in samples.chunks_exact_mut(CHANNELS) {
        let alpha = pixel[CHANNELS - 1].to_f64() / scale;
        for sample in &mut pixel[..CHANNELS - 1] {
            *sample = S::from_f64(sample.to_f64() * alpha);
        }
    }
}

fn unpremultiply<S: NativeSample, const CHANNELS: usize>(samples: &mut [S]) {
    let scale = S::alpha_max();
    for pixel in samples.chunks_exact_mut(CHANNELS) {
        let alpha = pixel[CHANNELS - 1].to_f64();
        if alpha == 0.0 { continue; }
        for sample in &mut pixel[..CHANNELS - 1] {
            *sample = S::from_f64(sample.to_f64() * scale / alpha);
        }
    }
}
