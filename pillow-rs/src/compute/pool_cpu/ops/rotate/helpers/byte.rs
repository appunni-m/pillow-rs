//! Native byte-channel Rotate math shared by exact mode owners.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::rotate::helpers::geometry::{
    discrete_angle, rotate_geometry, Geometry,
};
use crate::compute::pool_cpu::ops::rotate::helpers::nearest::{
    contiguous_three_byte_run_len, positive_step_in_bounds_span, fill_three_byte, fill_rgba,
};
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

pub(crate) fn execute<P, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions = (image.width(), image.height());
    if is_identity(request) {
        return Ok(image.clone());
    }
    if let Some(degree) = discrete_angle(request) {
        if degree == 180 || request.expand || dimensions.0 == dimensions.1 {
            return rotate_discrete::<P, CHANNELS>(image, degree);
        }
    }
    execute_affine::<P, CHANNELS, FORCE_NEAREST, ALPHA>(image, request)
}

fn is_identity(request: RotateRequest) -> bool {
    request.center.is_none()
        && request.translate.is_none()
        && request.angle.rem_euclid(360.0).abs() <= f64::EPSILON
}

fn execute_affine<P, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let geometry = rotate_geometry(image.width(), image.height(), request);
    let dimensions = CheckedDims::new(geometry.width, geometry.height, CHANNELS as u8)?;
    let mut output = dimensions.alloc_buffer();
    let nearest = FORCE_NEAREST || request.nearest;
    let filter = if nearest { ResampleFilter::Nearest } else { request.filter };
    let fill = request.fill.unwrap_or((0, 0, 0, 0));
    if nearest {
        affine_nearest::<CHANNELS>(
            image.as_raw(), (image.width(), image.height()), geometry, fill, &mut output,
        );
    } else {
        let mut source = image.as_raw().clone();
        if ALPHA {
            premultiply::<CHANNELS>(&mut source);
        }
        affine_filter::<CHANNELS>(
            &source,
            (image.width(), image.height()),
            geometry,
            fill,
            filter,
            &mut output,
        );
        if ALPHA {
            unpremultiply::<CHANNELS>(&mut output);
        }
    }
    crate::image_utils::typed_bytes_to_buffer::<P>(dimensions, output)
}

fn rotate_discrete<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    degree: u32,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let (width, height) = image.dimensions();
    let (output_width, output_height) = if degree == 180 {
        (width, height)
    } else {
        (height, width)
    };
    let dimensions = CheckedDims::new(output_width, output_height, CHANNELS as u8)?;
    let mut output = dimensions.alloc_buffer();
    for y in 0..output_height {
        for x in 0..output_width {
            let (source_x, source_y) = match degree {
                90 => (width - 1 - y, x),
                180 => (width - 1 - x, height - 1 - y),
                270 => (y, height - 1 - x),
                _ => unreachable!("discrete Rotate degree was validated"),
            };
            copy_pixel::<CHANNELS>(
                image.as_raw(), &mut output, width, output_width, source_x, source_y, x, y,
            );
        }
    }
    crate::image_utils::typed_bytes_to_buffer::<P>(dimensions, output)
}

fn copy_pixel<const CHANNELS: usize>(
    source: &[u8],
    output: &mut [u8],
    source_width: u32,
    output_width: u32,
    source_x: u32,
    source_y: u32,
    output_x: u32,
    output_y: u32,
) {
    let source_start = ((source_y * source_width + source_x) as usize) * CHANNELS;
    let output_start = ((output_y * output_width + output_x) as usize) * CHANNELS;
    output[output_start..output_start + CHANNELS]
        .copy_from_slice(&source[source_start..source_start + CHANNELS]);
}

pub(super) fn fill_sample<const CHANNELS: usize>(
    fill: (u8, u8, u8, u8),
    channel: usize,
) -> u8 {
    if CHANNELS == 2 && channel == 1 {
        fill.3
    } else {
        match channel {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        }
    }
}

fn affine_nearest<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
) {
    let [a, b, c, d, e, f] = geometry.affine;
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let (step_x_x, step_y_x, step_x_y, step_y_y) = (fixed(a), fixed(b), fixed(d), fixed(e));
    let (origin_x, origin_y) = (fixed(c + a * 0.5 + b * 0.5), fixed(f + d * 0.5 + e * 0.5));
    affine_nearest_rows::<CHANNELS>(
        source, source_size, geometry, fill, output,
        (origin_x, origin_y, step_x_x, step_y_x, step_x_y, step_y_y),
    );
}

fn affine_nearest_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    fixed: (i64, i64, i64, i64, i64, i64),
) {
    if geometry.width == 0 || geometry.height == 0 {
        return;
    }
    if CHANNELS == 3 {
        affine_nearest_rgb_rows::<CHANNELS>(
            source, source_size, geometry, fill, output, fixed,
        );
        return;
    }
    if CHANNELS == 4 {
        affine_nearest_rgba_rows::<CHANNELS>(
            source, source_size, geometry, fill, output, fixed,
        );
        return;
    }
    affine_nearest_generic_rows::<CHANNELS>(source, source_size, geometry, fill, output, fixed);
}

fn affine_nearest_generic_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    fixed: (i64, i64, i64, i64, i64, i64),
) {
    let output_stride = geometry.width as usize * CHANNELS;
    for (y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        affine_nearest_generic_row::<CHANNELS>(
            source, source_size, fill, fixed, y as u32, row,
        );
    }
}

fn affine_nearest_generic_row<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    fill: (u8, u8, u8, u8),
    fixed: (i64, i64, i64, i64, i64, i64),
    y: u32,
    row: &mut [u8],
) {
    let (source_width, source_height) = source_size;
    let (origin_x, origin_y, step_x_x, step_y_x, step_x_y, step_y_y) = fixed;
    let (mut sx, mut sy) = (
        origin_x + i64::from(y) * step_y_x,
        origin_y + i64::from(y) * step_y_y,
    );
    for (x, pixel) in row.chunks_exact_mut(CHANNELS).enumerate() {
        let (source_x, source_y) = (sx >> 16, sy >> 16);
        if source_x >= 0
            && source_x < i64::from(source_width)
            && source_y >= 0
            && source_y < i64::from(source_height)
        {
            let start = (source_y as u32 * source_width + source_x as u32) as usize * CHANNELS;
            pixel.copy_from_slice(&source[start..start + CHANNELS]);
        } else {
            for (channel, sample) in pixel.iter_mut().enumerate() {
                *sample = fill_sample::<CHANNELS>(fill, channel);
            }
        }
        if x + 1 < row.len() / CHANNELS {
            sx += step_x_x;
            sy += step_x_y;
        }
    }
}

fn affine_nearest_rgb_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    fixed: (i64, i64, i64, i64, i64, i64),
) {
    let (_, _, step_x_x, _, step_x_y, _) = fixed;
    let fill_pixel = [fill.0, fill.1, fill.2];
    let output_stride = geometry.width as usize * 3;
    for (y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let span = positive_step_in_bounds_span(
            y as u32, geometry.width, source_size, fixed,
        );
        let Some((start, end, mut source_x, mut source_y)) = span else {
            affine_nearest_generic_row::<CHANNELS>(
                source, source_size, fill, fixed, y as u32, row,
            );
            continue;
        };
        let (start_byte, end_byte) = (start * 3, end * 3);
        fill_three_byte(&mut row[..start_byte], fill_pixel);
        fill_three_byte(&mut row[end_byte..], fill_pixel);
        if !copy_contiguous_three_byte_runs(
            source, row, source_size.0, start_byte, end - start,
            &mut source_x, &mut source_y, step_x_x, step_x_y,
        ) {
            affine_nearest_generic_row::<CHANNELS>(
                source, source_size, fill, fixed, y as u32, row,
            );
        }
    }
}

fn copy_contiguous_three_byte_runs(
    source: &[u8],
    row: &mut [u8],
    source_width: u32,
    destination_start: usize,
    span_length: usize,
    source_x: &mut i64,
    source_y: &mut i64,
    step_x_x: i64,
    step_x_y: i64,
) -> bool {
    let mut offset = 0;
    while offset < span_length {
        let Some(run_length) = contiguous_three_byte_run_len(
            *source_x, *source_y, step_x_x, step_x_y, span_length - offset,
        ) else {
            return false;
        };
        if run_length < 4 {
            return false;
        }
        let source_start = (((*source_y >> 16) as usize * source_width as usize)
            + (*source_x >> 16) as usize)
            * 3;
        let destination = destination_start + offset * 3;
        let copy_bytes = run_length * 3;
        row[destination..destination + copy_bytes]
            .copy_from_slice(&source[source_start..source_start + copy_bytes]);
        offset += run_length;
        if offset < span_length
            && !advance_fixed_coordinates(source_x, source_y, step_x_x, step_x_y, run_length)
        {
            return false;
        }
    }
    true
}

fn advance_fixed_coordinates(
    source_x: &mut i64,
    source_y: &mut i64,
    step_x_x: i64,
    step_x_y: i64,
    count: usize,
) -> bool {
    let Ok(count) = i128::try_from(count) else {
        return false;
    };
    let Some(next_x) = i128::from(*source_x)
        .checked_add(i128::from(step_x_x) * count)
        .and_then(|value| i64::try_from(value).ok())
    else {
        return false;
    };
    let Some(next_y) = i128::from(*source_y)
        .checked_add(i128::from(step_x_y) * count)
        .and_then(|value| i64::try_from(value).ok())
    else {
        return false;
    };
    (*source_x, *source_y) = (next_x, next_y);
    true
}

fn affine_nearest_rgba_rows<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    output: &mut [u8],
    fixed: (i64, i64, i64, i64, i64, i64),
) {
    let output_stride = geometry.width as usize * 4;
    let fill_pixel = [fill.0, fill.1, fill.2, fill.3];
    for (y, row) in output.chunks_exact_mut(output_stride).enumerate() {
        let Some((start, end, source_x, source_y)) = positive_step_in_bounds_span(
            y as u32, geometry.width, source_size, fixed,
        ) else {
            affine_nearest_generic_row::<CHANNELS>(
                source, source_size, fill, fixed, y as u32, row,
            );
            continue;
        };
        let (start_byte, end_byte) = (start * 4, end * 4);
        fill_rgba(&mut row[..start_byte], fill_pixel);
        fill_rgba(&mut row[end_byte..], fill_pixel);
        copy_rgba_span(source, row, source_size.0, start_byte, end - start, source_x, source_y, fixed);
    }
}

fn copy_rgba_span(
    source: &[u8],
    row: &mut [u8],
    source_width: u32,
    destination_start: usize,
    span_length: usize,
    mut source_x: i64,
    mut source_y: i64,
    fixed: (i64, i64, i64, i64, i64, i64),
) {
    let (_, _, step_x_x, _, step_x_y, _) = fixed;
    let span = &mut row[destination_start..destination_start + span_length * 4];
    let groups = span_length / 4;
    let tail = span_length % 4;
    for (group_index, group) in span.chunks_exact_mut(16).enumerate() {
        for pixel_index in 0..4 {
            copy_rgba_pixel(source, group, pixel_index * 4, source_width, source_x, source_y);
            if group_index + 1 < groups || tail > 0 || pixel_index < 3 {
                source_x += step_x_x;
                source_y += step_x_y;
            }
        }
    }
    for (tail_index, pixel) in span.chunks_exact_mut(16).into_remainder().chunks_exact_mut(4).enumerate() {
        copy_rgba_pixel(source, pixel, 0, source_width, source_x, source_y);
        if tail_index + 1 < tail {
            source_x += step_x_x;
            source_y += step_x_y;
        }
    }
}

fn copy_rgba_pixel(
    source: &[u8],
    destination: &mut [u8],
    destination_offset: usize,
    source_width: u32,
    source_x: i64,
    source_y: i64,
) {
    let source_start = ((source_y >> 16) as usize * source_width as usize
        + (source_x >> 16) as usize)
        * 4;
    destination[destination_offset..destination_offset + 4]
        .copy_from_slice(&source[source_start..source_start + 4]);
}

fn affine_filter<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    geometry: Geometry,
    fill: (u8, u8, u8, u8),
    filter: ResampleFilter,
    output: &mut [u8],
) {
    let (width, height) = source_size;
    let [a, b, c, d, e, f] = geometry.affine;
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            let (sx, sy) = sample_coordinates(a, b, c, d, e, f, x, y);
            let start = (y * geometry.width + x) as usize * CHANNELS;
            if !in_filter_support(sx, sy, width, height) {
                write_fill::<CHANNELS>(output, start, fill);
            } else if matches!(filter, ResampleFilter::Bicubic) {
                sample_bicubic::<CHANNELS>(source, (width, height), sx, sy, output, start);
            } else {
                sample_bilinear::<CHANNELS>(source, (width, height), sx, sy, output, start);
            }
        }
    }
}

fn sample_coordinates(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, x: u32, y: u32) -> (f64, f64) {
    let (x, y) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
    (a.mul_add(x, b * y) + c - 0.5, d.mul_add(x, e * y) + f - 0.5)
}

fn in_filter_support(x: f64, y: f64, width: u32, height: u32) -> bool {
    x >= -0.5 && x < f64::from(width) - 0.5 && y >= -0.5 && y < f64::from(height) - 0.5
}

fn sample_bilinear<const CHANNELS: usize>(
    source: &[u8],
    size: (u32, u32),
    x: f64,
    y: f64,
    output: &mut [u8],
    start: usize,
) {
    let (width, height) = size;
    let (x, y) = (x.clamp(0.0, f64::from(width - 1)), y.clamp(0.0, f64::from(height - 1)));
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - f64::from(x0), y - f64::from(y0));
    for channel in 0..CHANNELS {
        let p00 = sample::<CHANNELS>(source, width, x0, y0, channel) as f64;
        let p10 = sample::<CHANNELS>(source, width, x1, y0, channel) as f64;
        let p01 = sample::<CHANNELS>(source, width, x0, y1, channel) as f64;
        let p11 = sample::<CHANNELS>(source, width, x1, y1, channel) as f64;
        let top = (p10 - p00).mul_add(fx, p00);
        let bottom = (p11 - p01).mul_add(fx, p01);
        output[start + channel] = (bottom - top).mul_add(fy, top) as u8;
    }
}

fn sample_bicubic<const CHANNELS: usize>(
    source: &[u8],
    size: (u32, u32),
    x: f64,
    y: f64,
    output: &mut [u8],
    start: usize,
) {
    let (width, height) = size;
    let floor_x = x.floor() as i64;
    let floor_y = y.floor() as i64;
    let base_x = floor_x - 1;
    let base_y = floor_y - 1;
    let (fx, fy) = (x - floor_x as f64, y - floor_y as f64);
    for channel in 0..CHANNELS {
        let mut rows = [0.0; 4];
        for (row, value) in rows.iter_mut().enumerate() {
            let source_y = (base_y + row as i64).clamp(0, i64::from(height - 1)) as u32;
            let taps = [0, 1, 2, 3].map(|tap| {
                let source_x = (base_x + tap).clamp(0, i64::from(width - 1)) as u32;
                sample::<CHANNELS>(source, width, source_x, source_y, channel) as f64
            });
            *value = crate::compute::pool_cpu::ops::rotate::helpers::cubic::horizontal_f64(taps, fx);
        }
        let value = crate::compute::pool_cpu::ops::rotate::helpers::cubic::horizontal_f64(rows, fy);
        output[start + channel] = value.clamp(0.0, 255.0) as u8;
    }
}

#[inline(always)]
fn sample<const CHANNELS: usize>(source: &[u8], width: u32, x: u32, y: u32, channel: usize) -> u8 {
    source[(y * width + x) as usize * CHANNELS + channel]
}

fn write_fill<const CHANNELS: usize>(output: &mut [u8], start: usize, fill: (u8, u8, u8, u8)) {
    for channel in 0..CHANNELS {
        output[start + channel] = fill_sample::<CHANNELS>(fill, channel);
    }
}

fn premultiply<const CHANNELS: usize>(samples: &mut [u8]) {
    for pixel in samples.chunks_exact_mut(CHANNELS) {
        let alpha = f64::from(pixel[CHANNELS - 1]) / 255.0;
        for sample in &mut pixel[..CHANNELS - 1] {
            *sample = (f64::from(*sample) * alpha + 0.5) as u8;
        }
    }
}

fn unpremultiply<const CHANNELS: usize>(samples: &mut [u8]) {
    for pixel in samples.chunks_exact_mut(CHANNELS) {
        let alpha = u16::from(pixel[CHANNELS - 1]);
        if alpha == 0 {
            continue;
        }
        for sample in &mut pixel[..CHANNELS - 1] {
            *sample = (u16::from(*sample) * 255 / alpha).min(255) as u8;
        }
    }
}
