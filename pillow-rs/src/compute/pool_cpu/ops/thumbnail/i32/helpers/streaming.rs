//! Stream ImageI32's 2x2 reducing-gap samples into the existing resize ring.

use crate::checked_dims::CheckedDims;
use crate::compute::helpers::resampling::rounding::round_up;
use crate::compute::pool_cpu::ops::resize::helpers::coefficients;
use crate::compute::pool_cpu::ops::resize::i32::helpers::streaming as resize_streaming;
use crate::error::PilError;
use crate::ops::pil_resize::{F64MulAdd, FilterCoeffsF64, i32_samples_from_le_bytes};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn reduce_source_row_pair(
    source: &[i32],
    source_width: usize,
    reduced_row: &mut [i32],
    reduced_y: usize,
) {
    let top_start = reduced_y * 2 * source_width;
    let bottom_start = top_start + source_width;
    let top = &source[top_start..top_start + source_width];
    let bottom = &source[bottom_start..bottom_start + source_width];
    for (output_x, reduced) in reduced_row.iter_mut().enumerate() {
        let source_x = output_x * 2;
        let quartet = top[source_x]
            .wrapping_add(top[source_x + 1])
            .wrapping_add(bottom[source_x])
            .wrapping_add(bottom[source_x + 1]);
        *reduced = round_up(f64::from(quartet) / 4.0) as i32;
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn horizontal_ring_row<F: F64MulAdd>(
    source: &[i32],
    source_width: usize,
    reduced_y: usize,
    reduced_row: &mut [i32],
    ring: &mut [i32],
    ring_mask: usize,
    destination_width: usize,
    horizontal: &FilterCoeffsF64,
    fixed_weights: Option<&(usize, Vec<[f64; 8]>)>,
    fma: &F,
) {
    reduce_source_row_pair(source, source_width, reduced_row, reduced_y);
    let ring_start = (reduced_y & ring_mask) * destination_width;
    let ring_row = &mut ring[ring_start..ring_start + destination_width];
    resize_streaming::horizontal_row(reduced_row, ring_row, horizontal, fixed_weights, fma);
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn vertical_output_row<F: F64MulAdd>(
    output: &mut [u8],
    output_y: usize,
    vertical: &FilterCoeffsF64,
    ring: &[i32],
    ring_mask: usize,
    destination_width: usize,
    fma: &F,
) {
    let first_source_y = vertical.xmin[output_y] as usize;
    let weights = &vertical.weights[output_y];
    if let Ok(weights) = <&[f64; 8]>::try_from(weights.as_slice()) {
        for (output_x, output_sample) in output.chunks_exact_mut(4).enumerate() {
            let accumulator = resize_streaming::sum_eight(
                weights,
                first_source_y,
                |source_y| ring[(source_y & ring_mask) * destination_width + output_x],
                fma,
            );
            output_sample
                .copy_from_slice(&resize_streaming::round_to_i32(accumulator).to_le_bytes());
        }
    } else {
        for (output_x, output_sample) in output.chunks_exact_mut(4).enumerate() {
            let accumulator = resize_streaming::sum_general(
                weights,
                first_source_y,
                |source_y| ring[(source_y & ring_mask) * destination_width + output_x],
                fma,
            );
            output_sample
                .copy_from_slice(&resize_streaming::round_to_i32(accumulator).to_le_bytes());
        }
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn execute_stream<F: F64MulAdd>(
    source: &[i32],
    source_dimensions: (u32, u32),
    destination: (u32, u32),
    horizontal: &FilterCoeffsF64,
    vertical: &FilterCoeffsF64,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let destination_width = destination.0 as usize;
    let maximum_vertical_taps = vertical.weights.iter().map(Vec::len).max().unwrap_or(0);
    let ring_rows = maximum_vertical_taps
        .checked_next_power_of_two()
        .ok_or_else(|| {
            PilError::ValueError("resize_i: vertical coefficient span is too large".into())
        })?;
    let ring_samples = ring_rows
        .checked_mul(destination_width)
        .ok_or_else(|| PilError::ValueError("resize_i: vertical ring is too large".into()))?;
    let mut ring = vec![0_i32; ring_samples];
    let reduced_width = (source_dimensions.0 / 2) as usize;
    let mut reduced_row = vec![0_i32; reduced_width];
    let output_dimensions = CheckedDims::new(destination.0, destination.1, 4)?;
    let output_stride = output_dimensions.row_stride();
    let mut output_bytes = output_dimensions.alloc_buffer();
    let fixed_weights = resize_streaming::contiguous_eight_tap_weights(&horizontal.weights);
    let source_width = source_dimensions.0 as usize;
    let ring_mask = ring_rows - 1;
    let mut next_source_row = 0;

    for (output_y, output_row) in output_bytes.chunks_mut(output_stride).enumerate() {
        let first_vertical_row = vertical.xmin[output_y] as usize;
        let source_end = first_vertical_row + vertical.weights[output_y].len();
        while next_source_row < source_end {
            horizontal_ring_row(
                source,
                source_width,
                next_source_row,
                &mut reduced_row,
                &mut ring,
                ring_mask,
                destination_width,
                horizontal,
                fixed_weights.as_ref(),
                fma,
            );
            next_source_row += 1;
        }
        vertical_output_row(
            output_row,
            output_y,
            vertical,
            &ring,
            ring_mask,
            destination_width,
            fma,
        );
    }

    RgbaImage::from_raw(destination.0, destination.1, output_bytes)
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute<F: F64MulAdd>(
    input: &RgbaImage,
    destination: (u32, u32),
    filter: ResampleFilter,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    let source_dimensions = input.dimensions();
    if source_dimensions.0 % 2 != 0 || source_dimensions.1 % 2 != 0 {
        return Err(PilError::InternalError(
            "streamed I thumbnail reduction requires even source dimensions".into(),
        ));
    }
    let source_dimensions =
        CheckedDims::new_allow_empty(source_dimensions.0, source_dimensions.1, 4)?;
    let source = i32_samples_from_le_bytes(input.as_raw(), source_dimensions.total_pixels());
    let source_dimensions = input.dimensions();
    let reduced_dimensions = (source_dimensions.0 / 2, source_dimensions.1 / 2);
    let (horizontal, vertical) =
        coefficients::for_resize(reduced_dimensions, destination, filter, None);
    execute_stream(
        &source,
        source_dimensions,
        destination,
        &horizontal,
        &vertical,
        fma,
    )
}
