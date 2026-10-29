//! Focused kernels for native-L SIMD BoxBlur.

use super::{
    SIMD_BOX_BLUR_BIAS, SIMD_BOX_BLUR_SCALE, simd_blur_load_u8x16, simd_blur_pack_u32x16_to_bytes,
    u16x16, u32x16,
};
use wide::u8x16;

#[inline(always)]
fn simd_luma_load_radius_one_block(source: &[u8], start: usize, edge: u8) -> u8x16 {
    if start >= source.len() {
        return u8x16::splat(edge);
    }
    let count = (source.len() - start).min(16);
    if count == 16 {
        return simd_blur_load_u8x16(source, start);
    }
    let mut bytes = [edge; 16];
    bytes[..count].copy_from_slice(&source[start..start + count]);
    u8x16::new(bytes)
}

/// Apply one native-L horizontal radius-one box pass with adjacent SIMD lanes.
///
/// Three overlapping unaligned vectors provide each block's left, center, and
/// right samples. The byte sum is widened before arithmetic, and the rounded
/// average uses an exact reciprocal multiply for every radius-one byte sum.
fn simd_luma_horizontal_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);

    let reciprocal = u32x16::splat(21_846);
    let one = u32x16::splat(1);

    for y in 0..height {
        let row_start = y * width;
        let row = &source[row_start..row_start + width];
        let output_row = &mut destination[row_start..row_start + width];
        let left_edge = row[0];
        let right_edge = row[width - 1];

        for x in (0..width).step_by(16) {
            let left = if x == 0 {
                let mut bytes = [left_edge; 16];
                let count = (width - 1).min(15);
                bytes[1..count + 1].copy_from_slice(&row[..count]);
                u8x16::new(bytes)
            } else {
                simd_luma_load_radius_one_block(row, x - 1, right_edge)
            };
            let center = simd_luma_load_radius_one_block(row, x, right_edge);
            let right = simd_luma_load_radius_one_block(row, x.saturating_add(1), right_edge);
            let left = u32x16::from(u16x16::from(left));
            let center = u32x16::from(u16x16::from(center));
            let right = u32x16::from(u16x16::from(right));
            let sum = left + center + right;
            let average = ((sum + one) * reciprocal) >> 16u32;
            let packed = simd_blur_pack_u32x16_to_bytes(average);
            let count = (width - x).min(16);
            output_row[x..x + count].copy_from_slice(&packed[..count]);
        }
    }
}

/// Apply one weighted radius-one horizontal GaussianBlur pass with SIMD lanes
/// spanning adjacent L pixels, including Pillow's fractional far-edge taps.
fn simd_luma_horizontal_radius_one_weighted_row(
    source: &[u8],
    destination: &mut [u8],
    whole_weight: u32,
    fractional_weight: u32,
) {
    let width = source.len();
    debug_assert_eq!(destination.len(), width);
    if width == 0 {
        return;
    }

    let right_edge = source[width - 1];
    let left_one_indices = u8x16::new([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    let left_two_indices = u8x16::new([0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);
    crate::compute::record_pipeline_operation_vector_blocks(width.div_ceil(16) as u64);

    for x in (0..width).step_by(16) {
        let center_bytes = simd_luma_load_radius_one_block(source, x, right_edge);
        let left_one_bytes = if x == 0 {
            center_bytes.swizzle_relaxed(left_one_indices)
        } else {
            simd_luma_load_radius_one_block(source, x - 1, right_edge)
        };
        let left_two_bytes = if x == 0 {
            center_bytes.swizzle_relaxed(left_two_indices)
        } else {
            simd_luma_load_radius_one_block(source, x - 2, right_edge)
        };
        let right_one_bytes = simd_luma_load_radius_one_block(source, x + 1, right_edge);
        let right_two_bytes = simd_luma_load_radius_one_block(source, x + 2, right_edge);

        let center = u32x16::from(u16x16::from(center_bytes));
        let neighbors = u32x16::from(u16x16::from(left_one_bytes))
            + center
            + u32x16::from(u16x16::from(right_one_bytes));
        let far_edges = u32x16::from(u16x16::from(left_two_bytes))
            + u32x16::from(u16x16::from(right_two_bytes));
        let weighted = neighbors * whole_weight + far_edges * fractional_weight + rounding_bias;
        let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
        let count = (width - x).min(16);
        destination[x..x + count].copy_from_slice(&packed[..count]);
    }
}

/// Run Pillow's three rounded L horizontal GaussianBlur passes one row at a
/// time, keeping the intermediate rows hot instead of writing two full-frame
/// images before the third pass.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_luma_horizontal_radius_one_three_passes_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
    scratch_a: &mut [u8],
    scratch_b: &mut [u8],
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    debug_assert_eq!(scratch_a.len(), width);
    debug_assert_eq!(scratch_b.len(), width);
    if width == 0 || height == 0 {
        return;
    }

    for y in 0..height {
        let start = y * width;
        let end = start + width;
        simd_luma_horizontal_radius_one_weighted_row(
            &source[start..end],
            scratch_a,
            whole_weight,
            fractional_weight,
        );
        simd_luma_horizontal_radius_one_weighted_row(
            scratch_a,
            scratch_b,
            whole_weight,
            fractional_weight,
        );
        simd_luma_horizontal_radius_one_weighted_row(
            scratch_b,
            &mut destination[start..end],
            whole_weight,
            fractional_weight,
        );
    }
}

/// Apply one native-L vertical radius-one box pass with vector lanes spanning
/// adjacent columns. Keep Pillow's sliding window across output rows: the
/// previous implementation reloaded three window rows plus both fractional
/// edge rows for every output row. The rolling row state reads only the
/// newly entering rows while preserving the unsigned fixed-point recurrence.
pub(in crate::compute::pool_simd::ops::adapters) fn simd_luma_vertical_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }

    if width == 0 || height == 0 {
        return;
    }

    let last_y = height - 1;
    let whole_weight = u32x16::splat(whole_weight);
    let fractional_weight = u32x16::splat(fractional_weight);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);

    for x in (0..width).step_by(16) {
        let count = (width - x).min(16);
        let load_row = |y: usize| {
            let mut bytes = [0u8; 16];
            let start = y * width + x;
            bytes[..count].copy_from_slice(&source[start..start + count]);
            u8x16::new(bytes)
        };

        // For output y, Pillow's leaving, center, entering, and fractional
        // far-edge samples are respectively max(y-2, 0), max(y-1, 0),
        // min(y+1, last), and min(y+2, last). Three initial copies of row 0
        // reproduce blur_line's radius-one accumulator before its first step.
        // Retain those byte rows as bytes: keeping five widened u32x16 rows
        // live beside the accumulator creates excessive vector-register
        // pressure on ARM64.
        let mut leaving = load_row(0);
        let mut center = load_row(0);
        let mut current = load_row(0);
        let mut entering = load_row(1.min(last_y));
        let mut far_edge = load_row(2.min(last_y));
        let mut accumulator = u32x16::from(u16x16::from(leaving)) * u32x16::splat(3);

        for y in 0..height {
            let leaving_wide = u32x16::from(u16x16::from(leaving));
            accumulator = accumulator - leaving_wide + u32x16::from(u16x16::from(entering));
            let edge_sum = leaving_wide + u32x16::from(u16x16::from(far_edge));
            let weighted =
                accumulator * whole_weight + edge_sum * fractional_weight + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            let output_start = y * width + x;
            destination[output_start..output_start + count].copy_from_slice(&packed[..count]);

            leaving = center;
            center = current;
            current = entering;
            entering = far_edge;
            far_edge = load_row(y.saturating_add(3).min(last_y));
        }
    }
}

/// Apply Pillow's horizontal and vertical radius-one BoxBlur passes to L rows.
///
/// The horizontal pass is rounded before the vertical pass, matching Pillow's
/// two-pass fixed-point behavior. Three horizontally filtered rows stay in a
/// ring, avoiding a full-frame input clone and horizontal intermediate.
pub(crate) fn simd_luma_box_blur_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let mut rows = [vec![0u8; width], vec![0u8; width], vec![0u8; width]];
    let mut next_horizontal_row = 0usize;
    let last_row = height - 1;
    let whole_weight = u32x16::splat(SIMD_BOX_BLUR_SCALE / 3);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);

    for y in 0..height {
        let bottom_row = y.saturating_add(1).min(last_row);
        while next_horizontal_row <= bottom_row {
            let source_start = next_horizontal_row * width;
            let source_row = &source[source_start..source_start + width];
            let slot = next_horizontal_row % 3;
            simd_luma_horizontal_radius_one_rows(source_row, &mut rows[slot], width, 1);
            next_horizontal_row += 1;
        }

        let top_row = y.saturating_sub(1);
        let top = &rows[top_row % 3];
        let center = &rows[y % 3];
        let bottom = &rows[bottom_row % 3];
        let output_start = y * width;
        let output_row = &mut destination[output_start..output_start + width];

        for x in (0..width).step_by(16) {
            let top = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                top,
                x,
                top[width - 1],
            )));
            let center = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                center,
                x,
                center[width - 1],
            )));
            let bottom = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                bottom,
                x,
                bottom[width - 1],
            )));
            let weighted = (top + center + bottom) * whole_weight + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            let count = (width - x).min(16);
            output_row[x..x + count].copy_from_slice(&packed[..count]);
        }
    }

    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
}
