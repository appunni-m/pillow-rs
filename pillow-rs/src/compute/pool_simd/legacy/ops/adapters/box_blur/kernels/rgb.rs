//! Native-format radius-one Gaussian and BoxBlur kernels.

use super::{
    CheckedDims, SIMD_BOX_BLUR_BIAS, SIMD_BOX_BLUR_SCALE, simd_blur_load_u8x16,
    simd_blur_pack_u32x16_to_bytes, u16x16, u32x16,
};
use wide::u8x16;
#[inline(always)]
fn simd_rgb_blur_scalar_sample(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    byte_index: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let x = byte_index / 3;
    let channel = byte_index % 3;
    let left = x.saturating_sub(1);
    let right = x.saturating_add(1).min(width - 1);
    let far_left = x.saturating_sub(2);
    let far_right = x.saturating_add(2).min(width - 1);
    let sample = |pixel: usize| u32::from(source[pixel * 3 + channel]);
    let sum = sample(left) + sample(x) + sample(right);
    let edge = if fractional_weight == 0 {
        0
    } else {
        sample(far_left) + sample(far_right)
    };
    let weighted = sum
        .wrapping_mul(whole_weight)
        .wrapping_add(edge.wrapping_mul(fractional_weight))
        .wrapping_add(SIMD_BOX_BLUR_BIAS);
    destination[byte_index] = (weighted >> 24) as u8;
}

#[inline]
fn simd_rgb_horizontal_radius_one_row(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    simd_rgb_horizontal_radius_one_row_with_vectors(
        input,
        output,
        width,
        whole_weight,
        fractional_weight,
        u32x16::splat(whole_weight),
        u32x16::splat(fractional_weight),
        u32x16::splat(SIMD_BOX_BLUR_BIAS),
    );
}

#[inline]
fn simd_rgb_horizontal_radius_one_row_with_vectors(
    input: &[u8],
    output: &mut [u8],
    width: usize,
    whole_weight: u32,
    fractional_weight: u32,
    whole_weight_vector: u32x16,
    fractional_weight_vector: u32x16,
    rounding_bias: u32x16,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert_eq!(input.len(), row_stride);
    debug_assert_eq!(output.len(), row_stride);

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    for byte_index in 0..prefix_end {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }

    let mut byte_index = prefix_end;
    if fractional_weight == 0 {
        while byte_index + 16 <= interior_end {
            // Three byte samples sum to at most 765, so add them as u16 lanes
            // and widen only the completed sum before Pillow's fixed-point
            // multiply. Radius one has no fractional edge contribution.
            let load = |offset: usize| u16x16::from(simd_blur_load_u8x16(input, offset));
            let sum = u32x16::from(load(byte_index - 3) + load(byte_index) + load(byte_index + 3));
            let weighted = sum * whole_weight_vector + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_index..byte_index + 16].copy_from_slice(&packed);
            byte_index += 16;
        }
    } else {
        while byte_index + 16 <= interior_end {
            let load = |offset: usize| u16x16::from(simd_blur_load_u8x16(input, offset));
            let sum = u32x16::from(load(byte_index - 3) + load(byte_index) + load(byte_index + 3));
            let edge = u32x16::from(load(byte_index - 6) + load(byte_index + 6));
            let weighted =
                sum * whole_weight_vector + edge * fractional_weight_vector + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            output[byte_index..byte_index + 16].copy_from_slice(&packed);
            byte_index += 16;
        }
    }

    for byte_index in byte_index..interior_end {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
    for byte_index in interior_end.max(prefix_end)..row_stride {
        simd_rgb_blur_scalar_sample(
            input,
            output,
            width,
            byte_index,
            whole_weight,
            fractional_weight,
        );
    }
}

/// Apply one horizontal radius-one RGB pass without the generic scalar
/// rolling recurrence. Three-byte neighbors are 3/6 bytes apart, so vector
/// loads may cross pixel boundaries while every lane still retains its own
/// channel. Replicated borders and the short vector tail use the scalar path.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_rgb_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let vector_blocks_per_row = interior_end.saturating_sub(prefix_end) / 16;
    let scalar_samples_per_row = row_stride.saturating_sub(vector_blocks_per_row * 16);
    crate::compute::record_pipeline_operation_vector_blocks(
        vector_blocks_per_row.saturating_mul(height) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        scalar_samples_per_row.saturating_mul(height) as u64,
    );

    for y in 0..height {
        let row_start = y * row_stride;
        simd_rgb_horizontal_radius_one_row(
            &source[row_start..row_start + row_stride],
            &mut destination[row_start..row_start + row_stride],
            width,
            whole_weight,
            fractional_weight,
        );
    }
}

/// Apply one RGB vertical radius-one pass in row-major three-byte storage.
///
/// A vertical blur treats every channel byte as an independent sample, so a
/// 16-byte vector may begin or end inside an RGB pixel. The same byte offset
/// is loaded from each neighboring row; no channel reordering or transpose is
/// needed. Tail vectors retain the exact fixed-point rounding.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_rgb_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());

    let last_y = height - 1;
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    for y in 0..height {
        let sum_top_base = y.saturating_sub(1) * row_stride;
        let sum_middle_base = y * row_stride;
        let sum_bottom_base = y.saturating_add(1).min(last_y) * row_stride;
        let edge_top_base = y.saturating_sub(2) * row_stride;
        let edge_bottom_base = y.saturating_add(2).min(last_y) * row_stride;
        let output_row = y * row_stride;

        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |base: usize| {
                if count == 16 {
                    u32x16::from(u16x16::from(simd_blur_load_u8x16(source, base + x)))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&source[base + x..base + x + count]);
                    u32x16::from(u16x16::from(u8x16::new(padded)))
                }
            };
            let sum = load(sum_top_base) + load(sum_middle_base) + load(sum_bottom_base);
            let edge = if fractional_weight == 0 {
                u32x16::splat(0)
            } else {
                load(edge_top_base) + load(edge_bottom_base)
            };
            let weighted = sum * u32x16::splat(whole_weight)
                + edge * u32x16::splat(fractional_weight)
                + u32x16::splat(SIMD_BOX_BLUR_BIAS);
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output_row + x..output_row + x + count].copy_from_slice(&packed[..count]);
        }
    }
}

/// Stream horizontal RGB BoxBlur(1) rows through a three-row SIMD ring before
/// applying Pillow's rounded vertical pass. The horizontal bytes stay in
/// cache-sized row storage instead of a full-frame intermediate.
#[derive(Clone, Copy)]
struct RgbBoxBlurRadiusOnePlan {
    width: usize,
    height: usize,
    row_stride: usize,
    ring_rows: usize,
    horizontal_weight: u32,
    horizontal_fractional_weight: u32,
    horizontal_weight_vector: u32x16,
    horizontal_fractional_weight_vector: u32x16,
    horizontal_rounding_bias: u32x16,
    vertical_weight: u32x16,
}

#[inline]
fn simd_rgb_box_blur_stream_rows(
    source: &[u8],
    destination: &mut [u8],
    ring: &mut [u8],
    plan: &RgbBoxBlurRadiusOnePlan,
) {
    let RgbBoxBlurRadiusOnePlan {
        width,
        height,
        row_stride,
        ring_rows,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
        vertical_weight,
    } = *plan;
    let mut previous_slot = 0usize;
    let mut current_slot = 0usize;
    let mut next_slot = 1usize;
    let bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    for y in 0..height {
        let previous = previous_slot * row_stride;
        let current = current_slot * row_stride;
        let next = next_slot * row_stride;
        let output = y * row_stride;
        for x in (0..row_stride).step_by(16) {
            let count = (row_stride - x).min(16);
            let load = |start: usize| {
                if count == 16 {
                    u16x16::from(simd_blur_load_u8x16(ring, start + x))
                } else {
                    let mut padded = [0u8; 16];
                    padded[..count].copy_from_slice(&ring[start + x..start + x + count]);
                    u16x16::from(u8x16::new(padded))
                }
            };
            let sum = u32x16::from(load(previous) + load(current) + load(next));
            let weighted = sum * vertical_weight + bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            destination[output + x..output + x + count].copy_from_slice(&packed[..count]);
        }

        if y + 1 < height {
            previous_slot = current_slot;
            current_slot = next_slot;
            if y + 2 < height {
                next_slot = (next_slot + 1) % ring_rows;
                let source_y = y + 2;
                let source_start = source_y * row_stride;
                let ring_start = next_slot * row_stride;
                simd_rgb_horizontal_radius_one_row_with_vectors(
                    &source[source_start..source_start + row_stride],
                    &mut ring[ring_start..ring_start + row_stride],
                    width,
                    horizontal_weight,
                    horizontal_fractional_weight,
                    horizontal_weight_vector,
                    horizontal_fractional_weight_vector,
                    horizontal_rounding_bias,
                );
            } else {
                next_slot = current_slot;
            }
        }
    }
}

pub(in crate::compute::pool_simd::ops::adapters::box_blur) fn simd_rgb_box_blur_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    horizontal_weight: u32,
    horizontal_fractional_weight: u32,
    vertical_weight: u32,
    vertical_fractional_weight: u32,
) {
    let row_stride = width
        .checked_mul(3)
        .expect("validated RGB dimensions fit the native row stride");
    debug_assert!(width > 0);
    debug_assert!(height > 0);
    debug_assert_eq!(source.len(), row_stride * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_eq!(horizontal_weight, SIMD_BOX_BLUR_SCALE / 3);
    debug_assert_eq!(horizontal_fractional_weight, 0);
    debug_assert_eq!(vertical_weight, SIMD_BOX_BLUR_SCALE / 3);
    debug_assert_eq!(vertical_fractional_weight, 0);

    let interior_end = width.saturating_sub(2) * 3;
    let prefix_end = width.min(2) * 3;
    let horizontal_blocks_per_row = interior_end.saturating_sub(prefix_end) / 16;
    let horizontal_scalar_bytes = row_stride.saturating_sub(horizontal_blocks_per_row * 16);
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks_per_row.saturating_mul(height) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_scalar_bytes.saturating_mul(height) as u64,
    );

    if height == 1 {
        simd_rgb_horizontal_radius_one_row(
            source,
            destination,
            width,
            horizontal_weight,
            horizontal_fractional_weight,
        );
        return;
    }

    crate::compute::record_pipeline_operation_vector_blocks(
        row_stride.div_ceil(16).saturating_mul(height) as u64,
    );
    let ring_rows = height.min(3);
    let ring_dimensions = CheckedDims::new(
        u32::try_from(width).expect("validated RGB width fits u32"),
        u32::try_from(ring_rows).expect("three-row RGB ring height fits u32"),
        3,
    )
    .expect("validated RGB row ring dimensions fit image limits");
    let mut ring = ring_dimensions.alloc_buffer();
    let horizontal_weight_vector = u32x16::splat(horizontal_weight);
    let horizontal_fractional_weight_vector = u32x16::splat(horizontal_fractional_weight);
    let horizontal_rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    simd_rgb_horizontal_radius_one_row_with_vectors(
        &source[..row_stride],
        &mut ring[..row_stride],
        width,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
    );
    simd_rgb_horizontal_radius_one_row_with_vectors(
        &source[row_stride..row_stride * 2],
        &mut ring[row_stride..row_stride * 2],
        width,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
    );

    let plan = RgbBoxBlurRadiusOnePlan {
        width,
        height,
        row_stride,
        ring_rows,
        horizontal_weight,
        horizontal_fractional_weight,
        horizontal_weight_vector,
        horizontal_fractional_weight_vector,
        horizontal_rounding_bias,
        vertical_weight: u32x16::splat(vertical_weight),
    };
    simd_rgb_box_blur_stream_rows(source, destination, &mut ring, &plan);
}
