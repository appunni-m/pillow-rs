//! Portable-wide native-L 3x3 order-statistic kernel retained from the SIMD adapter.

use crate::error::PilError;
use crate::raster::GrayImage;
use wide::u8x16;

#[inline(always)]
fn compare_exchange(values: &mut [u8x16; 9], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

#[inline(always)]
fn median_of_nine(mut values: [u8x16; 9]) -> u8x16 {
    compare_exchange(&mut values, 1, 2);
    compare_exchange(&mut values, 4, 5);
    compare_exchange(&mut values, 7, 8);
    compare_exchange(&mut values, 0, 1);
    compare_exchange(&mut values, 3, 4);
    compare_exchange(&mut values, 6, 7);
    compare_exchange(&mut values, 0, 3);
    compare_exchange(&mut values, 1, 2);
    compare_exchange(&mut values, 4, 5);
    compare_exchange(&mut values, 7, 8);
    compare_exchange(&mut values, 3, 6);
    compare_exchange(&mut values, 4, 7);
    compare_exchange(&mut values, 5, 8);
    compare_exchange(&mut values, 1, 4);
    compare_exchange(&mut values, 2, 5);
    compare_exchange(&mut values, 4, 7);
    compare_exchange(&mut values, 4, 2);
    compare_exchange(&mut values, 6, 4);
    compare_exchange(&mut values, 4, 2);
    values[4]
}

#[inline(always)]
fn second_order_statistic(values: &[u8x16; 9]) -> u8x16 {
    let mut smallest = values[0].min(values[1]);
    let mut second = values[0].max(values[1]);
    for &value in values.iter().skip(2) {
        let lower = smallest.min(value);
        let upper = smallest.max(value);
        second = second.min(upper);
        smallest = lower;
    }
    second
}

#[inline(always)]
fn luma_window_vector(
    source_row: &[u8],
    width: usize,
    x_start: usize,
    active: usize,
    column_offset: usize,
) -> u8x16 {
    if active == 16 && x_start > 0 && x_start.saturating_add(16) < width {
        let start = x_start + column_offset - 1;
        let mut lanes = [0u8; 16];
        lanes.copy_from_slice(&source_row[start..start + 16]);
        return u8x16::new(lanes);
    }

    let mut lanes = [0u8; 16];
    for (lane, value) in lanes.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(1)
            .min(width - 1);
        *value = source_row[source_x];
    }
    u8x16::new(lanes)
}

fn filter_luma_row(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    rank: u32,
    y: usize,
) {
    let top = y.saturating_sub(1) * width;
    let middle = y * width;
    let bottom = y.saturating_add(1).min(height - 1) * width;
    let row_starts = [top, middle, bottom];
    let mut x = 0usize;

    while x < width {
        let active = (width - x).min(16);
        let mut values = [u8x16::splat(0); 9];
        for (row_index, row_start) in row_starts.into_iter().enumerate() {
            let source_row = &raw[row_start..row_start + width];
            for column_offset in 0..3 {
                values[row_index * 3 + column_offset] =
                    luma_window_vector(source_row, width, x, active, column_offset);
            }
        }
        let selected = if rank == 1 {
            second_order_statistic(&values)
        } else {
            median_of_nine(values)
        }
        .to_array();
        output[x..x + active].copy_from_slice(&selected[..active]);
        x += active;
    }
}

pub(super) fn execute(input: &GrayImage, rank: u32) -> Result<GrayImage, PilError> {
    if !matches!(rank, 1 | 4) {
        return Err(PilError::InternalError(
            "L 3x3 rank strategy received an unsupported rank".into(),
        ));
    }
    let width = input.width() as usize;
    let height = input.height() as usize;
    let expected = width.checked_mul(height).ok_or_else(|| {
        PilError::InternalError("L rank dimensions overflow the native buffer".into())
    })?;
    if width == 0 || height == 0 || input.as_raw().len() != expected {
        return Err(PilError::InternalError(
            "L rank strategy received invalid native dimensions".into(),
        ));
    }

    let vector_blocks = (width.div_ceil(16) as u64).saturating_mul(height as u64);
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
        return GrayImage::from_raw(input.width(), input.height(), output)
            .ok_or_else(|| PilError::ValueError("rank output buffer does not match L mode".into()));
    }

    let mut output = vec![0u8; expected];
    for y in 0..height {
        let row_start = y * width;
        filter_luma_row(
            input.as_raw(),
            &mut output[row_start..row_start + width],
            width,
            height,
            rank,
            y,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    GrayImage::from_raw(input.width(), input.height(), output)
        .ok_or_else(|| PilError::ValueError("rank output buffer does not match L mode".into()))
}
