//! Portable-wide BoxBlur recurrence used by exact native-byte strategies.
//!
//! Each exact-mode owner instantiates its physical channel count as a const
//! generic, keeping channel layout out of the row kernel's runtime inputs.

use crate::checked_dims::CheckedDims;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use wide::u32x8;

const LANES: usize = 8;
const BOX_BLUR_BIAS: u32 = super::super::BOX_BLUR_BIAS;

#[derive(Clone, Copy)]
struct BlurAxis {
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
}

#[derive(Clone, Copy)]
struct BlurPlan {
    horizontal: BlurAxis,
    vertical: BlurAxis,
    passes: u32,
}

#[derive(Clone, Copy)]
enum Region {
    Leading,
    Middle,
    Trailing,
    Clamped,
}

pub(super) fn execute<const CHANNELS: usize>(
    image: &DynamicImage,
    mode: Mode,
    request: crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
    selection: &StrategySelection,
    context: &ExecutionContext,
) -> Result<DynamicImage, PilError> {
    super::validate_native_byte_blur_selection(selection, mode, request, context)?;
    validate_strategy::<CHANNELS>(image, mode)?;
    let dimensions = CheckedDims::new(image.width(), image.height(), CHANNELS as u8)?;
    if image.as_bytes().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "native-byte blur input does not match its checked dimensions".into(),
        ));
    }
    if is_identity_copy(request) {
        return super::copy_native_byte_blur_input(image, mode, CHANNELS);
    }
    let plan = blur_plan(request)?;
    if crate::compute::pool_cpu::ops::filter::helpers::native_small_uniform_bytes(
        image.as_bytes(),
        image.width(),
        image.height(),
        CHANNELS,
        crate::compute::pool_cpu::ops::filter::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        return super::copy_native_byte_blur_input(image, mode, CHANNELS);
    }

    let horizontal = execute_axis::<CHANNELS>(
        image.as_bytes(),
        dimensions.width as usize,
        dimensions.height as usize,
        plan.passes,
        plan.horizontal,
    )?;
    let output = execute_vertical_axis::<CHANNELS>(
        &horizontal,
        dimensions.width as usize,
        dimensions.height as usize,
        plan.passes,
        plan.vertical,
    )?;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
        mode,
        image.width(),
        image.height(),
        output,
    )
    .ok_or_else(|| PilError::InternalError("native-byte blur output mode shape mismatch".into()))
}

fn is_identity_copy(request: crate::compute::pool_cpu::ops::filter::ops::FilterRequest) -> bool {
    use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
    match request {
        FilterRequest::GaussianBlur { sigma } => sigma == 0.0,
        FilterRequest::BoxBlur { radius } => radius == 0,
        FilterRequest::BoxBlurXY {
            radius_x, radius_y, ..
        } => radius_x == 0.0 && radius_y == 0.0,
        _ => false,
    }
}

fn validate_strategy<const CHANNELS: usize>(
    image: &DynamicImage,
    mode: Mode,
) -> Result<(), PilError> {
    if image.mode() != mode {
        return Err(PilError::InternalError(
            "native-byte blur strategy received a different raster mode".into(),
        ));
    }
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "native-byte blur strategy received an unsupported layout".into(),
        ));
    }
    Ok(())
}

fn blur_plan(
    request: crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
) -> Result<BlurPlan, PilError> {
    use crate::compute::pool_cpu::ops::filter::ops::FilterRequest;
    let (horizontal_radius, vertical_radius, passes) = match request {
        FilterRequest::GaussianBlur { sigma } if sigma.is_finite() && sigma != 0.0 => {
            let axis = gaussian_axis(sigma);
            (axis, axis, 3)
        }
        FilterRequest::BoxBlur { radius } if radius != 0 => {
            let axis = box_axis(radius as f32);
            (axis, axis, 1)
        }
        FilterRequest::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } if radius_x.is_finite()
            && radius_y.is_finite()
            && radius_x >= 0.0
            && radius_y >= 0.0
            && passes != 0
            && (radius_x != 0.0 || radius_y != 0.0) =>
        {
            (box_axis(radius_x), box_axis(radius_y), passes)
        }
        _ => {
            return Err(PilError::InternalError(
                "native-byte blur strategy received an ineligible request".into(),
            ));
        }
    };
    Ok(BlurPlan {
        horizontal: horizontal_radius,
        vertical: vertical_radius,
        passes,
    })
}

fn box_axis(radius: f32) -> BlurAxis {
    let (radius, whole_weight, fractional_weight) =
        crate::compute::helpers::filters::box_blur_parameters(radius);
    BlurAxis {
        radius,
        whole_weight,
        fractional_weight,
    }
}

fn gaussian_axis(sigma: f32) -> BlurAxis {
    let (radius, whole_weight, fractional_weight) =
        crate::compute::helpers::filters::gaussian_blur_box_parameters(sigma);
    BlurAxis {
        radius,
        whole_weight,
        fractional_weight,
    }
}

fn execute_axis<const CHANNELS: usize>(
    source: &[u8],
    width: usize,
    height: usize,
    passes: u32,
    axis: BlurAxis,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8)?;
    let mut work = source.to_vec();
    let mut scratch = dimensions.alloc_buffer();
    for _ in 0..passes {
        blur_rows::<CHANNELS>(&work, &mut scratch, width, height, axis)?;
        std::mem::swap(&mut work, &mut scratch);
    }
    Ok(work)
}

fn execute_vertical_axis<const CHANNELS: usize>(
    source: &[u8],
    width: usize,
    height: usize,
    passes: u32,
    axis: BlurAxis,
) -> Result<Vec<u8>, PilError> {
    let transposed_dimensions = CheckedDims::new(height as u32, width as u32, CHANNELS as u8)?;
    let mut work = transposed_dimensions.alloc_buffer();
    transpose::<CHANNELS>(source, &mut work, width, height)?;
    let mut scratch = transposed_dimensions.alloc_buffer();
    for _ in 0..passes {
        blur_rows::<CHANNELS>(&work, &mut scratch, height, width, axis)?;
        std::mem::swap(&mut work, &mut scratch);
    }

    let output_dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8)?;
    let mut output = output_dimensions.alloc_buffer();
    transpose::<CHANNELS>(&work, &mut output, height, width)?;
    Ok(output)
}

fn blur_rows<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    axis: BlurAxis,
) -> Result<(), PilError> {
    let dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "native-byte blur pass buffers do not match their dimensions".into(),
        ));
    }
    let row_stride = dimensions.row_stride();
    for row in 0..height {
        let start = row * row_stride;
        blur_line::<CHANNELS>(
            &source[start..start + row_stride],
            &mut destination[start..start + row_stride],
            width,
            axis,
        );
    }
    let blocks = blocks_per_line(width, axis.radius).saturating_mul(height as u64);
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    Ok(())
}

fn blocks_per_line(line_length: usize, radius: usize) -> u64 {
    let edge_a = radius.saturating_add(1).min(line_length);
    let edge_b = line_length.saturating_sub(radius.saturating_add(1));
    let ranges = if edge_a <= edge_b {
        [(0, edge_a), (edge_a, edge_b), (edge_b, line_length)]
    } else {
        [(0, edge_b), (edge_b, edge_a), (edge_a, line_length)]
    };
    ranges
        .into_iter()
        .map(|(start, end)| end.saturating_sub(start).div_ceil(LANES) as u64)
        .sum()
}

fn blur_line<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    line_length: usize,
    axis: BlurAxis,
) {
    let last = line_length - 1;
    let edge_a = axis.radius.saturating_add(1).min(line_length);
    let edge_b = line_length.saturating_sub(axis.radius.saturating_add(1));
    let mut accumulator = [0u32; CHANNELS];
    initialize_accumulator::<CHANNELS>(source, line_length, axis.radius, &mut accumulator);
    let mut apply = |start: usize, end: usize, region| {
        let mut output = start;
        while output < end {
            let count = (end - output).min(LANES);
            blur_block::<CHANNELS>(
                source,
                destination,
                &mut accumulator,
                axis,
                last,
                output,
                count,
                region,
            );
            output += count;
        }
    };
    if edge_a <= edge_b {
        apply(0, edge_a, Region::Leading);
        apply(edge_a, edge_b, Region::Middle);
        apply(edge_b, line_length, Region::Trailing);
    } else {
        apply(0, edge_b, Region::Leading);
        apply(edge_b, edge_a, Region::Clamped);
        apply(edge_a, line_length, Region::Trailing);
    }
}

fn initialize_accumulator<const CHANNELS: usize>(
    source: &[u8],
    line_length: usize,
    radius: usize,
    accumulator: &mut [u32; CHANNELS],
) {
    let edge_a = radius.saturating_add(1).min(line_length);
    let last_base = (line_length - 1) * CHANNELS;
    for channel in 0..CHANNELS {
        accumulator[channel] = u32::from(source[channel]).wrapping_mul((radius + 1) as u32);
    }
    for position in 0..edge_a.saturating_sub(1) {
        let base = position * CHANNELS;
        for channel in 0..CHANNELS {
            accumulator[channel] =
                accumulator[channel].wrapping_add(u32::from(source[base + channel]));
        }
    }
    let last_count = radius.saturating_add(1).saturating_sub(edge_a);
    for channel in 0..CHANNELS {
        accumulator[channel] = accumulator[channel]
            .wrapping_add(u32::from(source[last_base + channel]).wrapping_mul(last_count as u32));
    }
}

fn blur_block<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    axis: BlurAxis,
    last: usize,
    output_start: usize,
    output_count: usize,
    region: Region,
) {
    if axis.fractional_weight == 0 {
        blur_block_no_fractional::<CHANNELS>(
            source,
            destination,
            accumulator,
            axis,
            last,
            output_start,
            output_count,
            region,
        );
        return;
    }

    let mut sums = [[0u32; LANES]; CHANNELS];
    let mut fractions = [[0u32; LANES]; CHANNELS];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add, far_left, far_right) =
            sample_indices(region, output, axis.radius, last);
        for channel in 0..CHANNELS {
            accumulator[channel] = accumulator[channel]
                .wrapping_sub(u32::from(source[subtract * CHANNELS + channel]))
                .wrapping_add(u32::from(source[add * CHANNELS + channel]));
            sums[channel][lane] = accumulator[channel];
            fractions[channel][lane] = u32::from(source[far_left * CHANNELS + channel])
                .wrapping_add(u32::from(source[far_right * CHANNELS + channel]))
                .wrapping_mul(axis.fractional_weight);
        }
    }
    store_blur_block::<CHANNELS>(
        destination,
        axis,
        output_start,
        output_count,
        &sums,
        &fractions,
    );
}

fn blur_block_no_fractional<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; CHANNELS],
    axis: BlurAxis,
    last: usize,
    output_start: usize,
    output_count: usize,
    region: Region,
) {
    let mut sums = [[0u32; LANES]; CHANNELS];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add, _, _) = sample_indices(region, output, axis.radius, last);
        for channel in 0..CHANNELS {
            accumulator[channel] = accumulator[channel]
                .wrapping_sub(u32::from(source[subtract * CHANNELS + channel]))
                .wrapping_add(u32::from(source[add * CHANNELS + channel]));
            sums[channel][lane] = accumulator[channel];
        }
    }

    let bias = u32x8::splat(BOX_BLUR_BIAS);
    let weight = u32x8::splat(axis.whole_weight);
    for channel in 0..CHANNELS {
        let weighted = (u32x8::new(sums[channel]) * weight + bias) >> 24u32;
        let values = weighted.to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * CHANNELS + channel] = values[lane] as u8;
        }
    }
}

fn sample_indices(
    region: Region,
    output: usize,
    radius: usize,
    last: usize,
) -> (usize, usize, usize, usize) {
    match region {
        Region::Leading => (0, output + radius, 0, output + radius + 1),
        Region::Middle => (
            output - radius - 1,
            output + radius,
            output - radius - 1,
            output + radius + 1,
        ),
        Region::Trailing => (output - radius - 1, last, output - radius - 1, last),
        Region::Clamped => (0, last, 0, last),
    }
}

fn store_blur_block<const CHANNELS: usize>(
    destination: &mut [u8],
    axis: BlurAxis,
    output_start: usize,
    output_count: usize,
    sums: &[[u32; LANES]; CHANNELS],
    fractions: &[[u32; LANES]; CHANNELS],
) {
    for channel in 0..CHANNELS {
        let weighted = u32x8::new(sums[channel]) * u32x8::splat(axis.whole_weight)
            + u32x8::new(fractions[channel]);
        let values = ((weighted + u32x8::splat(BOX_BLUR_BIAS)) >> 24u32).to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * CHANNELS + channel] = values[lane] as u8;
        }
    }
}

fn transpose<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
) -> Result<(), PilError> {
    let source_dimensions = CheckedDims::new(width as u32, height as u32, CHANNELS as u8)?;
    let destination_dimensions = CheckedDims::new(height as u32, width as u32, CHANNELS as u8)?;
    if source.len() != source_dimensions.total_bytes()
        || destination.len() != destination_dimensions.total_bytes()
    {
        return Err(PilError::InternalError(
            "native-byte blur transpose buffers do not match their dimensions".into(),
        ));
    }
    let source_stride = source_dimensions.row_stride();
    let destination_stride = destination_dimensions.row_stride();
    for x in 0..width {
        let destination_start = x * destination_stride;
        for y in 0..height {
            let source_start = y * source_stride + x * CHANNELS;
            let output_start = destination_start + y * CHANNELS;
            destination[output_start..output_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }
    Ok(())
}
