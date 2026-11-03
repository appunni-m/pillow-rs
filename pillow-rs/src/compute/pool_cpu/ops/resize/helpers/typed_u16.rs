//! Native u16 filtering for internal multi-channel Resize modes.

use super::{coefficients, typed_nearest::BoxBounds};
use crate::compute::helpers::resampling::rounding::round_up;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn resize<P: Pixel<Subpixel = u16>>(
    input: &ImageBuffer<P, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
) -> ImageBuffer<P, Vec<u16>> {
    if matches!(filter, ResampleFilter::Nearest) {
        return super::typed_nearest::resize(input, destination_width, destination_height, bounds);
    }
    let (source_width, source_height) = input.dimensions();
    let mut output = ImageBuffer::<P, Vec<u16>>::new(destination_width, destination_height);
    if source_width == 0 || source_height == 0 || destination_width == 0 || destination_height == 0 {
        return output;
    }

    let channel_count = usize::from(P::CHANNEL_COUNT);
    let (horizontal, vertical) = coefficients::for_resize(
        (source_width, source_height),
        (destination_width, destination_height),
        filter,
        bounds,
    );
    let mut intermediate = ImageBuffer::<P, Vec<u16>>::new(destination_width, source_height);
    for source_y in 0..source_height {
        let source_start = source_y as usize * source_width as usize * channel_count;
        let source_end = source_start + source_width as usize * channel_count;
        let intermediate_start = source_y as usize * destination_width as usize * channel_count;
        let intermediate_end = intermediate_start + destination_width as usize * channel_count;
        horizontal_pass(
            &input.as_raw()[source_start..source_end],
            &horizontal,
            destination_width,
            channel_count,
            &mut intermediate.as_mut()[intermediate_start..intermediate_end],
        );
    }

    for destination_y in 0..destination_height {
        let output_start = destination_y as usize * destination_width as usize * channel_count;
        let output_end = output_start + destination_width as usize * channel_count;
        vertical_pass(
            intermediate.as_raw(),
            &vertical,
            destination_y as usize,
            destination_width,
            source_height,
            channel_count,
            &mut output.as_mut()[output_start..output_end],
        );
    }
    output
}

pub(crate) fn resize_with_alpha<P: Pixel<Subpixel = u16>>(
    input: &ImageBuffer<P, Vec<u16>>,
    destination_width: u32,
    destination_height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
) -> ImageBuffer<P, Vec<u16>> {
    if matches!(filter, ResampleFilter::Nearest) {
        return resize(input, destination_width, destination_height, filter, bounds);
    }
    let channel_count = usize::from(P::CHANNEL_COUNT);
    let Some(mut premultiplied) = ImageBuffer::<P, Vec<u16>>::from_raw(
        input.width(),
        input.height(),
        input.as_raw().to_vec(),
    ) else {
        unreachable!("cloned typed image retains its validated sample count");
    };
    premultiply_u16(&mut premultiplied, channel_count);
    let mut output = resize(
        &premultiplied,
        destination_width,
        destination_height,
        filter,
        bounds,
    );
    unpremultiply_u16(&mut output, channel_count);
    output
}

fn premultiply_u16(image: &mut ImageBuffer<impl Pixel<Subpixel = u16>, Vec<u16>>, channels: usize) {
    let denominator = u64::from(u16::MAX);
    for pixel in image.chunks_exact_mut(channels) {
        let alpha = u64::from(pixel[channels - 1]);
        for sample in &mut pixel[..channels - 1] {
            let product = u64::from(*sample) * alpha + denominator / 2;
            *sample = (product / denominator) as u16;
        }
    }
}

fn unpremultiply_u16(
    image: &mut ImageBuffer<impl Pixel<Subpixel = u16>, Vec<u16>>,
    channels: usize,
) {
    let full_scale = u64::from(u16::MAX);
    for pixel in image.chunks_exact_mut(channels) {
        let alpha = u64::from(pixel[channels - 1]);
        if alpha == 0 {
            pixel[..channels - 1].fill(0);
            continue;
        }
        for sample in &mut pixel[..channels - 1] {
            let unscaled = (u64::from(*sample) * full_scale + alpha / 2) / alpha;
            *sample = unscaled.min(full_scale) as u16;
        }
    }
}

fn horizontal_pass(
    source_row: &[u16],
    coefficients: &crate::ops::pil_resize::FilterCoeffsF64,
    output_width: u32,
    channel_count: usize,
    output_row: &mut [u16],
) {
    for output_x in 0..output_width as usize {
        if coefficients.count[output_x] == 0 {
            continue;
        }
        let first = coefficients.xmin[output_x];
        for channel in 0..channel_count {
            let mut accumulator = 0.0f64;
            for (tap, &weight) in coefficients.weights[output_x].iter().enumerate() {
                let source_x = (first + tap as i64) as usize;
                accumulator += f64::from(source_row[source_x * channel_count + channel]) * weight;
            }
            output_row[output_x * channel_count + channel] = native_u16(accumulator);
        }
    }
}

fn vertical_pass(
    intermediate: &[u16],
    coefficients: &crate::ops::pil_resize::FilterCoeffsF64,
    output_y: usize,
    output_width: u32,
    source_height: u32,
    channel_count: usize,
    output_row: &mut [u16],
) {
    if coefficients.count[output_y] == 0 {
        return;
    }
    let first = coefficients.xmin[output_y];
    let intermediate_stride = output_width as usize * channel_count;
    for output_x in 0..output_width as usize {
        for channel in 0..channel_count {
            let mut accumulator = 0.0f64;
            for (tap, &weight) in coefficients.weights[output_y].iter().enumerate() {
                let source_y = (first + tap as i64) as usize;
                debug_assert!(source_y < source_height as usize);
                let sample = intermediate
                    [source_y * intermediate_stride + output_x * channel_count + channel];
                accumulator += f64::from(sample) * weight;
            }
            output_row[output_x * channel_count + channel] = native_u16(accumulator);
        }
    }
}

fn native_u16(value: f64) -> u16 {
    round_up(value).clamp(0.0, f64::from(u16::MAX)) as u16
}
