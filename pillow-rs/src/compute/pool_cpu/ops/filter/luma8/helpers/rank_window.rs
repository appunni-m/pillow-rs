//! Portable-wide byte order statistics for native L windows larger than 3x3.

use crate::error::PilError;
use crate::raster::GrayImage;
use wide::u8x16;

const LANES: usize = 16;
const MAX_AREA: usize = 225;
const MAX_SIZE: u32 = 15;

// TODO(pillow-rs#3ptb): freeze L rank output, edge, and fallback receipts. The
// preserved Apple ARM64 9x9 rank-40 path measured slower than serial CPU;
// revisit with an algorithm-focused benchmark after parity, keeping semantics.

#[inline]
fn sort_vectors(values: &mut [u8x16]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            values[index] = left.min(right);
            values[index + 1] = left.max(right);
            index += 2;
        }
    }
}

#[inline]
fn select_vector_order_statistic(values: &[u8x16], rank: u8) -> u8x16 {
    let mut lower = u8x16::splat(0);
    let mut upper = u8x16::splat(u8::MAX);
    let rank = u8x16::splat(rank);
    let one = u8x16::splat(1);

    for _ in 0..u8::BITS {
        let midpoint = lower + ((upper - lower) >> 1u32);
        let mut count = u8x16::splat(0);
        for &value in values {
            count += value.simd_le(midpoint).select(one, u8x16::splat(0));
        }
        let above_rank = count.simd_gt(rank);
        upper = above_rank.select(midpoint, upper);
        lower = above_rank.select(lower, midpoint + one);
    }
    lower
}

#[inline]
fn luma_window_vector(
    source_row: &[u8],
    width: usize,
    x_start: usize,
    active: usize,
    half: usize,
    column_offset: usize,
) -> u8x16 {
    let mut lanes = [0u8; LANES];
    for (lane, value) in lanes.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        *value = source_row[source_x];
    }
    u8x16::new(lanes)
}

fn filter_row(
    raw: &[u8],
    output_row: &mut [u8],
    width: usize,
    height: usize,
    size: usize,
    half: usize,
    area: usize,
    rank: usize,
    y: usize,
) {
    let mut x = 0usize;
    while x < width {
        let active = (width - x).min(LANES);
        let mut values = [u8x16::splat(0); MAX_AREA];
        let mut value_index = 0usize;
        for row_offset in 0..size {
            let source_y = y
                .saturating_add(row_offset)
                .saturating_sub(half)
                .min(height - 1);
            let row_start = source_y * width;
            let source_row = &raw[row_start..row_start + width];
            for column_offset in 0..size {
                values[value_index] =
                    luma_window_vector(source_row, width, x, active, half, column_offset);
                value_index += 1;
            }
        }
        let selected = if size <= 5 {
            sort_vectors(&mut values[..area]);
            values[rank]
        } else {
            select_vector_order_statistic(&values[..area], rank as u8)
        }
        .to_array();
        output_row[x..x + active].copy_from_slice(&selected[..active]);
        x += active;
    }
}

pub(in crate::compute::pool_cpu::ops::filter::luma8) fn execute(
    input: &GrayImage,
    size: u32,
    rank: u32,
) -> Result<GrayImage, PilError> {
    let Some(area) = (size as usize).checked_mul(size as usize) else {
        return Err(PilError::InternalError("L rank window overflow".into()));
    };
    if size == 0 || size % 2 == 0 || size > MAX_SIZE || rank as usize >= area {
        return Err(PilError::InternalError(
            "L rank received an unsupported window".into(),
        ));
    }
    let width = input.width() as usize;
    let height = input.height() as usize;
    let Some(expected) = width.checked_mul(height) else {
        return Err(PilError::InternalError("L rank dimensions overflow".into()));
    };
    if width == 0 || height == 0 || expected != input.as_raw().len() {
        return Err(PilError::InternalError(
            "L rank received invalid native dimensions".into(),
        ));
    }
    let vector_blocks = width.div_ceil(LANES) as u64 * height as u64;
    if crate::compute::pool_cpu::ops::filter::helpers::native_small_uniform_bytes(
        input.as_raw(),
        input.width(),
        input.height(),
        1,
        crate::compute::pool_cpu::ops::filter::helpers::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        let mut output = vec![0u8; expected];
        let (copy_blocks, scalar_tail) =
            crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                input.as_raw(),
                &mut output,
            )
            .ok_or_else(|| PilError::InternalError("L rank identity copy shape mismatch".into()))?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(copy_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return GrayImage::from_raw(input.width(), input.height(), output).ok_or_else(|| {
            PilError::ValueError("rank output buffer does not match L mode".into())
        });
    }

    let size = size as usize;
    let mut output = vec![0u8; expected];
    for y in 0..height {
        let row_start = y * width;
        let row_end = row_start + width;
        filter_row(
            input.as_raw(),
            &mut output[row_start..row_end],
            width,
            height,
            size,
            size / 2,
            area,
            rank as usize,
            y,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    GrayImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| PilError::ValueError("rank output buffer does not match L mode".into()))
}
