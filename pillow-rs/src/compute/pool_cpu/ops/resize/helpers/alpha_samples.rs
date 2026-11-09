//! Serial kernels for straight-alpha byte modes with identical premultiplication.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{
    FilterCoeffs, compact_resize_coeffs_to_source_span, fixed_point_to_u8, precompute_coeffs,
    precompute_coeffs_boxed_for_filter,
};
use crate::pipeline::ResampleFilter;

use super::byte_samples;
use super::coefficients::{
    byte_coefficients_fit_i32, byte_coefficients_fit_source, fixed_point_to_u8_i32,
};

type Bounds = Option<(f64, f64, f64, f64)>;

struct AlphaResizePlan<'a> {
    source: &'a [u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &'a FilterCoeffs,
    vertical: &'a FilterCoeffs,
    need_horizontal: bool,
    need_vertical: bool,
}

pub(crate) fn resize<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: Bounds,
) -> Result<Vec<u8>, PilError> {
    if !matches!(CHANNELS, 2 | 4) {
        return Err(PilError::DimensionError(
            "straight-alpha Resize requires two or four channels".into(),
        ));
    }
    let channels = CHANNELS as u8;
    let source_dims = CheckedDims::new_allow_empty(source_size.0, source_size.1, channels)?;
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "straight-alpha Resize received an invalid source buffer".into(),
        ));
    }
    let destination_dims =
        CheckedDims::new_allow_empty(destination_size.0, destination_size.1, channels)?;
    if matches!(filter, ResampleFilter::Nearest) {
        return byte_samples::resize::<CHANNELS>(
            source,
            source_size,
            destination_size,
            filter,
            bounds,
        );
    }
    if bounds.is_none() && source_size == destination_size {
        return Ok(source.to_vec());
    }
    if let Some(bounds) = bounds
        && is_full_box_identity(source_size, destination_size, bounds)
    {
        return Ok(source.to_vec());
    }
    if is_empty(source_size, destination_size) {
        return Ok(destination_dims.alloc_buffer());
    }
    if has_zero_fast_path(source, source_dims.total_pixels(), bounds) {
        return Ok(destination_dims.alloc_buffer());
    }
    match bounds {
        Some(bounds) => {
            filtered_boxed::<CHANNELS>(source, source_size, destination_size, filter, bounds)
        }
        None => filtered_standard::<CHANNELS>(source, source_size, destination_size, filter),
    }
}

fn is_empty(source_size: (u32, u32), destination_size: (u32, u32)) -> bool {
    source_size.0 == 0 || source_size.1 == 0 || destination_size.0 == 0 || destination_size.1 == 0
}

fn is_full_box_identity(
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> bool {
    let (left, top, right, bottom) = bounds;
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    source_size == destination_size
        && left == 0.0
        && top == 0.0
        && right == source_size.0 as f32
        && bottom == source_size.1 as f32
}

fn has_zero_fast_path(source: &[u8], source_pixels: usize, bounds: Bounds) -> bool {
    let pixels_are_eligible = match bounds {
        None => source_pixels != 0,
        Some(_) => source_pixels != 0 && source_pixels <= 256 * 256,
    };
    pixels_are_eligible && source.iter().all(|&sample| sample == 0)
}

fn filtered_standard<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
) -> Result<Vec<u8>, PilError> {
    let horizontal = precompute_coeffs(destination_size.0, source_size.0, filter);
    let vertical = precompute_coeffs(destination_size.1, source_size.1, filter);
    let coefficients_fit = byte_coefficients_fit_source(&horizontal, source_size.0 as usize)
        && byte_coefficients_fit_source(&vertical, source_size.1 as usize)
        && byte_coefficients_fit_i32(&horizontal)
        && byte_coefficients_fit_i32(&vertical);
    if coefficients_fit
        && let Some(output) = filtered_standard_i32::<CHANNELS>(
            source,
            source_size,
            destination_size,
            &horizontal,
            &vertical,
        )
    {
        return Ok(output);
    }
    filtered_with_coefficients::<CHANNELS>(AlphaResizePlan {
        source,
        source_size,
        destination_size,
        horizontal: &horizontal,
        vertical: &vertical,
        need_horizontal: true,
        need_vertical: true,
    })
}

fn filtered_boxed<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
) -> Result<Vec<u8>, PilError> {
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let (left, top, right, bottom) = bounds;
    let horizontal =
        precompute_coeffs_boxed_for_filter(destination_width, source_width, left, right, filter);
    let mut vertical =
        precompute_coeffs_boxed_for_filter(destination_height, source_height, top, bottom, filter);
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    let need_horizontal =
        destination_width != source_width || left != 0.0 || right != destination_width as f32;
    let need_vertical =
        destination_height != source_height || top != 0.0 || bottom != destination_height as f32;
    let (first_source_row, intermediate_rows) = if need_vertical {
        compact_resize_coeffs_to_source_span(&mut vertical, source_height)
            .unwrap_or((0, source_height))
    } else {
        (0, source_height)
    };
    let row_stride = source_width as usize * CHANNELS;
    let source_start = first_source_row as usize * row_stride;
    let source_len = intermediate_rows as usize * row_stride;
    filtered_with_coefficients::<CHANNELS>(AlphaResizePlan {
        source: &source[source_start..source_start + source_len],
        source_size: (source_width, intermediate_rows),
        destination_size,
        horizontal: &horizontal,
        vertical: &vertical,
        need_horizontal,
        need_vertical,
    })
}

fn filtered_standard_i32<const CHANNELS: usize>(
    source: &[u8],
    source_size: (u32, u32),
    destination_size: (u32, u32),
    horizontal: &FilterCoeffs,
    vertical: &FilterCoeffs,
) -> Option<Vec<u8>> {
    let (source_width, source_height) = source_size;
    let (destination_width, destination_height) = destination_size;
    let source_dims = CheckedDims::new(source_width, source_height, CHANNELS as u8).ok()?;
    let intermediate_dims =
        CheckedDims::new(destination_width, source_height, CHANNELS as u8).ok()?;
    let output_dims =
        CheckedDims::new(destination_width, destination_height, CHANNELS as u8).ok()?;
    if horizontal.xmin.len() != destination_width as usize
        || vertical.xmin.len() != destination_height as usize
        || source.len() != source_dims.total_bytes()
    {
        return None;
    }
    let source_stride = source_width as usize * CHANNELS;
    let output_stride = destination_width as usize * CHANNELS;
    let mut intermediate = intermediate_dims.alloc_buffer();
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height as usize {
        let source_start = source_y * source_stride;
        let output_start = source_y * output_stride;
        premultiply_row::<CHANNELS>(
            &source[source_start..source_start + source_stride],
            &mut premultiplied_row,
        );
        horizontal_row_i32::<CHANNELS>(
            &premultiplied_row,
            horizontal,
            &mut intermediate[output_start..output_start + output_stride],
        );
    }
    let mut output = output_dims.alloc_buffer();
    vertical_rows_i32::<CHANNELS>(
        &intermediate,
        destination_width as usize,
        destination_height as usize,
        vertical,
        &mut output,
    );
    Some(output)
}

fn horizontal_row_i32<const CHANNELS: usize>(
    source: &[u8],
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    let output_width = output.len() / CHANNELS;
    for output_x in 0..output_width {
        let weights = weights(coefficients, output_x);
        if weights.is_empty() {
            continue;
        }
        let first_source = coefficients.xmin[output_x] as usize * CHANNELS;
        for channel in 0..CHANNELS {
            let mut sum = 0_i32;
            for (tap, &weight) in weights.iter().enumerate() {
                sum += i32::from(source[first_source + tap * CHANNELS + channel]) * weight as i32;
            }
            output[output_x * CHANNELS + channel] = fixed_point_to_u8_i32(sum);
        }
    }
}

fn vertical_rows_i32<const CHANNELS: usize>(
    intermediate: &[u8],
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    let row_stride = output_width * CHANNELS;
    for output_y in 0..output_height {
        let weights = weights(coefficients, output_y);
        if weights.is_empty() {
            continue;
        }
        let first_source = coefficients.xmin[output_y] as usize * row_stride;
        let output_start = output_y * row_stride;
        for output_x in 0..output_width {
            let mut sums = [0_i32; CHANNELS];
            for (tap, &weight) in weights.iter().enumerate() {
                let source_start = first_source + tap * row_stride + output_x * CHANNELS;
                for (channel, sum) in sums.iter_mut().enumerate() {
                    *sum += i32::from(intermediate[source_start + channel]) * weight as i32;
                }
            }
            write_unpremultiplied_i32::<CHANNELS>(
                &sums,
                &mut output[output_start + output_x * CHANNELS..][..CHANNELS],
            );
        }
    }
}

fn write_unpremultiplied_i32<const CHANNELS: usize>(sums: &[i32; CHANNELS], output: &mut [u8]) {
    let alpha = fixed_point_to_u8_i32(sums[CHANNELS - 1]);
    for channel in 0..CHANNELS - 1 {
        output[channel] = unpremultiply_channel(fixed_point_to_u8_i32(sums[channel]), alpha);
    }
    output[CHANNELS - 1] = alpha;
}

fn filtered_with_coefficients<const CHANNELS: usize>(
    plan: AlphaResizePlan<'_>,
) -> Result<Vec<u8>, PilError> {
    let (source_width, source_height) = plan.source_size;
    let (destination_width, destination_height) = plan.destination_size;
    let channels = CHANNELS as u8;
    let output_dims = CheckedDims::new(destination_width, destination_height, channels)?;
    let intermediate_dims = CheckedDims::new(destination_width, source_height, channels)?;
    let source_stride = source_width as usize * CHANNELS;
    let output_stride = destination_width as usize * CHANNELS;
    let mut intermediate = intermediate_dims.alloc_buffer();
    let mut premultiplied_row = vec![0; source_stride];
    for source_y in 0..source_height as usize {
        let source_start = source_y * source_stride;
        let output_start = source_y * output_stride;
        let source_row = &plan.source[source_start..source_start + source_stride];
        let intermediate_row = &mut intermediate[output_start..output_start + output_stride];
        premultiply_row::<CHANNELS>(source_row, &mut premultiplied_row);
        if plan.need_horizontal {
            horizontal_row::<CHANNELS>(&premultiplied_row, plan.horizontal, intermediate_row);
        } else {
            intermediate_row.copy_from_slice(&premultiplied_row);
        }
    }
    let mut output = output_dims.alloc_buffer();
    if !plan.need_vertical {
        unpremultiply_image::<CHANNELS>(&intermediate, &mut output);
    } else if byte_samples::should_transpose_vertical(source_height, destination_width) {
        let transposed = byte_samples::transpose_samples::<CHANNELS>(
            &intermediate,
            source_height,
            destination_width,
            intermediate_dims,
        );
        vertical_transposed::<CHANNELS>(
            &transposed,
            source_height as usize,
            destination_width as usize,
            destination_height as usize,
            plan.vertical,
            &mut output,
        );
    } else {
        vertical_row_major::<CHANNELS>(
            &intermediate,
            destination_width as usize,
            destination_height as usize,
            plan.vertical,
            &mut output,
        );
    }
    Ok(output)
}

fn premultiply_row<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
    for (source_pixel, output_pixel) in source
        .chunks_exact(CHANNELS)
        .zip(output.chunks_exact_mut(CHANNELS))
    {
        let alpha = source_pixel[CHANNELS - 1];
        for channel in 0..CHANNELS - 1 {
            output_pixel[channel] = premultiply_channel(source_pixel[channel], alpha);
        }
        output_pixel[CHANNELS - 1] = alpha;
    }
}

fn horizontal_row<const CHANNELS: usize>(
    source: &[u8],
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    for (output_x, output_pixel) in output.chunks_exact_mut(CHANNELS).enumerate() {
        let weights = weights(coefficients, output_x);
        if weights.is_empty() {
            continue;
        }
        let first_source = coefficients.xmin[output_x] as usize * CHANNELS;
        for channel in 0..CHANNELS {
            let mut sum = 0_i64;
            for (tap, &weight) in weights.iter().enumerate() {
                sum += i64::from(source[first_source + tap * CHANNELS + channel]) * weight;
            }
            output_pixel[channel] = fixed_point_to_u8(sum);
        }
    }
}

fn vertical_row_major<const CHANNELS: usize>(
    intermediate: &[u8],
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    let row_stride = output_width * CHANNELS;
    for output_y in 0..output_height {
        let weights = weights(coefficients, output_y);
        let output_start = output_y * row_stride;
        let output_row = &mut output[output_start..output_start + row_stride];
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source = coefficients.xmin[output_y] as usize * row_stride;
        for output_x in 0..output_width {
            let source_start = first_source + output_x * CHANNELS;
            let destination_start = output_x * CHANNELS;
            vertical_pixel::<CHANNELS>(
                intermediate,
                source_start,
                row_stride,
                weights,
                &mut output_row[destination_start..destination_start + CHANNELS],
            );
        }
    }
}

fn vertical_transposed<const CHANNELS: usize>(
    intermediate: &[u8],
    source_height: usize,
    output_width: usize,
    output_height: usize,
    coefficients: &FilterCoeffs,
    output: &mut [u8],
) {
    let row_stride = output_width * CHANNELS;
    for output_y in 0..output_height {
        let weights = weights(coefficients, output_y);
        let output_start = output_y * row_stride;
        let output_row = &mut output[output_start..output_start + row_stride];
        if weights.is_empty() {
            output_row.fill(0);
            continue;
        }
        let first_source = coefficients.xmin[output_y] as usize;
        for output_x in 0..output_width {
            let source_start = (output_x * source_height + first_source) * CHANNELS;
            let destination_start = output_x * CHANNELS;
            vertical_pixel::<CHANNELS>(
                intermediate,
                source_start,
                CHANNELS,
                weights,
                &mut output_row[destination_start..destination_start + CHANNELS],
            );
        }
    }
}

fn vertical_pixel<const CHANNELS: usize>(
    source: &[u8],
    source_start: usize,
    tap_stride: usize,
    weights: &[i64],
    output: &mut [u8],
) {
    let mut sums = [0_i64; CHANNELS];
    for (tap, &weight) in weights.iter().enumerate() {
        let pixel_start = source_start + tap * tap_stride;
        for (channel, sum) in sums.iter_mut().enumerate() {
            *sum += i64::from(source[pixel_start + channel]) * weight;
        }
    }
    let alpha = fixed_point_to_u8(sums[CHANNELS - 1]);
    for channel in 0..CHANNELS - 1 {
        output[channel] = unpremultiply_channel(fixed_point_to_u8(sums[channel]), alpha);
    }
    output[CHANNELS - 1] = alpha;
}

fn unpremultiply_image<const CHANNELS: usize>(source: &[u8], output: &mut [u8]) {
    for (source_pixel, output_pixel) in source
        .chunks_exact(CHANNELS)
        .zip(output.chunks_exact_mut(CHANNELS))
    {
        let alpha = source_pixel[CHANNELS - 1];
        for channel in 0..CHANNELS - 1 {
            output_pixel[channel] = unpremultiply_channel(source_pixel[channel], alpha);
        }
        output_pixel[CHANNELS - 1] = alpha;
    }
}

fn premultiply_channel(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return value;
    }
    (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8
}

fn weights(coefficients: &FilterCoeffs, index: usize) -> &[i64] {
    let start = coefficients.offsets[index];
    &coefficients.weights[start..start + coefficients.count[index]]
}
